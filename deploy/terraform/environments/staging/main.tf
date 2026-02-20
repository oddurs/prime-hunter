# Staging environment — minimal Hetzner K3s cluster for testing.
#
# Single-worker cluster for CI/CD validation and pre-production testing.
# Uses the cheapest Hetzner instances to minimize cost.

terraform {
  backend "s3" {
    bucket = "darkreach-terraform-state"
    key    = "staging/terraform.tfstate"
    region = "us-east-1"
  }
}

variable "hcloud_token" {
  type      = string
  sensitive = true
}

# Minimal cluster: 1 control plane + 1 worker
module "hetzner" {
  source = "../../modules/hetzner-k3s"

  hcloud_token       = var.hcloud_token
  worker_count       = 1
  worker_type        = "cpx21"
  control_plane_type = "cx22"
  location           = "fsn1"
}

# Shared add-ons (monitoring optional in staging)
module "shared" {
  source = "../../modules/shared"

  install_keda          = true
  install_monitoring    = false
  install_cert_manager  = false
  install_ingress_nginx = true

  depends_on = [module.hetzner]
}

output "control_plane_ip" {
  value = module.hetzner.control_plane_ip
}
