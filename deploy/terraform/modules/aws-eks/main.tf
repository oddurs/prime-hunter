# AWS EKS cluster for darkreach burst capacity.
#
# Uses EKS with managed node groups and spot instances for cost-effective
# burst compute. c6a.2xlarge spot instances provide ~60-90% savings over
# on-demand pricing for CPU-bound prime hunting workloads.
#
# This module is intended for burst campaigns alongside the primary
# Hetzner cluster, providing elastic capacity for large search jobs.

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "region" {
  type    = string
  default = "us-east-1"
}

variable "cluster_name" {
  type    = string
  default = "darkreach"
}

variable "vpc_cidr" {
  type    = string
  default = "10.1.0.0/16"
}

variable "spot_instance_types" {
  type    = list(string)
  default = ["c6a.2xlarge", "c6i.2xlarge", "c5a.2xlarge"]
}

variable "spot_max_nodes" {
  type    = number
  default = 10
}

provider "aws" {
  region = var.region
}

# VPC for EKS
module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 5.0"

  name = "${var.cluster_name}-vpc"
  cidr = var.vpc_cidr

  azs             = ["${var.region}a", "${var.region}b"]
  private_subnets = ["10.1.1.0/24", "10.1.2.0/24"]
  public_subnets  = ["10.1.101.0/24", "10.1.102.0/24"]

  enable_nat_gateway   = true
  single_nat_gateway   = true
  enable_dns_hostnames = true

  public_subnet_tags = {
    "kubernetes.io/role/elb" = 1
  }
  private_subnet_tags = {
    "kubernetes.io/role/internal-elb" = 1
  }
}

# EKS cluster
module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = "1.29"

  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnets

  cluster_endpoint_public_access = true

  eks_managed_node_groups = {
    # Small on-demand group for coordinator
    coordinator = {
      instance_types = ["t3.medium"]
      min_size       = 1
      max_size       = 1
      desired_size   = 1

      labels = {
        role = "coordinator"
      }
    }

    # Spot instance group for compute workers
    workers-spot = {
      instance_types = var.spot_instance_types
      capacity_type  = "SPOT"
      min_size       = 0
      max_size       = var.spot_max_nodes
      desired_size   = 0

      labels = {
        role = "worker"
      }

      taints = [{
        key    = "spot"
        value  = "true"
        effect = "NO_SCHEDULE"
      }]
    }
  }
}

output "cluster_endpoint" {
  value = module.eks.cluster_endpoint
}

output "cluster_name" {
  value = module.eks.cluster_name
}
