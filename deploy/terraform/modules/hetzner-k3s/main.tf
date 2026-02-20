# Hetzner Cloud K3s cluster for darkreach.
#
# Deploys a lightweight K3s cluster on Hetzner Cloud for cost-effective
# CPU-bound prime hunting. Hetzner offers the best price/core ratio
# for compute workloads in Europe.
#
# Architecture:
# - 1 control plane (CX22: 2 vCPU, 4GB RAM, EUR ~4/mo)
# - N worker nodes (CPX41: 8 vCPU, 16GB RAM, EUR ~19/mo each)
# - Private network for inter-node communication
# - Firewall: SSH + K3s API + HTTP/HTTPS only

terraform {
  required_providers {
    hcloud = {
      source  = "hetznercloud/hcloud"
      version = "~> 1.45"
    }
  }
}

variable "hcloud_token" {
  type      = string
  sensitive = true
}

variable "ssh_key_name" {
  type    = string
  default = "darkreach"
}

variable "location" {
  type    = string
  default = "fsn1"
}

variable "worker_count" {
  type    = number
  default = 2
}

variable "worker_type" {
  type    = string
  default = "cpx41"
}

variable "control_plane_type" {
  type    = string
  default = "cx22"
}

provider "hcloud" {
  token = var.hcloud_token
}

# SSH key for node access
data "hcloud_ssh_key" "default" {
  name = var.ssh_key_name
}

# Private network for cluster communication
resource "hcloud_network" "cluster" {
  name     = "darkreach-cluster"
  ip_range = "10.0.0.0/16"
}

resource "hcloud_network_subnet" "nodes" {
  network_id   = hcloud_network.cluster.id
  type         = "cloud"
  network_zone = "eu-central"
  ip_range     = "10.0.1.0/24"
}

# Firewall
resource "hcloud_firewall" "cluster" {
  name = "darkreach-cluster"

  rule {
    direction = "in"
    protocol  = "tcp"
    port      = "22"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
  rule {
    direction = "in"
    protocol  = "tcp"
    port      = "6443"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
  rule {
    direction = "in"
    protocol  = "tcp"
    port      = "80"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
  rule {
    direction = "in"
    protocol  = "tcp"
    port      = "443"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
}

# Control plane node
resource "hcloud_server" "control_plane" {
  name        = "darkreach-cp"
  server_type = var.control_plane_type
  image       = "ubuntu-22.04"
  location    = var.location
  ssh_keys    = [data.hcloud_ssh_key.default.id]
  firewall_ids = [hcloud_firewall.cluster.id]

  user_data = <<-EOF
    #!/bin/bash
    curl -sfL https://get.k3s.io | sh -s - server \
      --disable traefik \
      --write-kubeconfig-mode 644 \
      --tls-san $(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4)
  EOF

  network {
    network_id = hcloud_network.cluster.id
    ip         = "10.0.1.10"
  }

  depends_on = [hcloud_network_subnet.nodes]
}

# Worker nodes
resource "hcloud_server" "worker" {
  count       = var.worker_count
  name        = "darkreach-worker-${count.index}"
  server_type = var.worker_type
  image       = "ubuntu-22.04"
  location    = var.location
  ssh_keys    = [data.hcloud_ssh_key.default.id]
  firewall_ids = [hcloud_firewall.cluster.id]

  user_data = <<-EOF
    #!/bin/bash
    # Wait for control plane to be ready
    sleep 30
    TOKEN=$(ssh -o StrictHostKeyChecking=no root@10.0.1.10 cat /var/lib/rancher/k3s/server/node-token)
    curl -sfL https://get.k3s.io | K3S_URL=https://10.0.1.10:6443 K3S_TOKEN=$TOKEN sh -
  EOF

  network {
    network_id = hcloud_network.cluster.id
    ip         = "10.0.1.${count.index + 20}"
  }

  depends_on = [hcloud_server.control_plane, hcloud_network_subnet.nodes]
}

output "control_plane_ip" {
  value = hcloud_server.control_plane.ipv4_address
}

output "worker_ips" {
  value = hcloud_server.worker[*].ipv4_address
}
