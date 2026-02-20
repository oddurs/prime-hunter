# Production environment — Hetzner K3s primary + AWS EKS burst.
#
# Hetzner is the primary cluster (best $/core for CPU-bound work).
# AWS spot instances provide elastic burst capacity for large campaigns.

terraform {
  backend "s3" {
    bucket = "darkreach-terraform-state"
    key    = "production/terraform.tfstate"
    region = "us-east-1"
  }
}

variable "hcloud_token" {
  type      = string
  sensitive = true
}

variable "enable_aws_burst" {
  type    = bool
  default = false
}

# Primary cluster: Hetzner K3s
module "hetzner" {
  source = "../../modules/hetzner-k3s"

  hcloud_token       = var.hcloud_token
  worker_count       = 3
  worker_type        = "cpx41"
  control_plane_type = "cx22"
  location           = "fsn1"
}

# Shared add-ons on primary cluster
module "shared" {
  source = "../../modules/shared"

  install_keda          = true
  install_monitoring    = true
  install_cert_manager  = true
  install_ingress_nginx = true

  depends_on = [module.hetzner]
}

# Optional: AWS burst cluster
module "aws" {
  count  = var.enable_aws_burst ? 1 : 0
  source = "../../modules/aws-eks"

  cluster_name    = "darkreach-burst"
  spot_max_nodes  = 20
  region          = "us-east-1"
}

output "hetzner_control_plane_ip" {
  value = module.hetzner.control_plane_ip
}

output "hetzner_worker_ips" {
  value = module.hetzner.worker_ips
}
