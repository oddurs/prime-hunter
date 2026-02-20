# Shared Kubernetes add-ons for darkreach clusters.
#
# Installs KEDA (auto-scaling), monitoring (prometheus-stack),
# cert-manager (TLS), and ingress-nginx via Helm provider.
# Applied to any K8s cluster (Hetzner K3s or AWS EKS).

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.12"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.25"
    }
  }
}

variable "kubeconfig_path" {
  type    = string
  default = "~/.kube/config"
}

variable "install_keda" {
  type    = bool
  default = true
}

variable "install_monitoring" {
  type    = bool
  default = true
}

variable "install_cert_manager" {
  type    = bool
  default = true
}

variable "install_ingress_nginx" {
  type    = bool
  default = true
}

# KEDA — Kubernetes Event-Driven Autoscaling
resource "helm_release" "keda" {
  count = var.install_keda ? 1 : 0

  name             = "keda"
  repository       = "https://kedacore.github.io/charts"
  chart            = "keda"
  version          = "2.13.0"
  namespace        = "keda"
  create_namespace = true
}

# kube-prometheus-stack — Prometheus + Grafana + Alertmanager
resource "helm_release" "monitoring" {
  count = var.install_monitoring ? 1 : 0

  name             = "monitoring"
  repository       = "https://prometheus-community.github.io/helm-charts"
  chart            = "kube-prometheus-stack"
  version          = "56.0.0"
  namespace        = "monitoring"
  create_namespace = true

  values = [file("${path.module}/../../../helm/monitoring-values.yaml")]
}

# cert-manager — Automatic TLS certificate management
resource "helm_release" "cert_manager" {
  count = var.install_cert_manager ? 1 : 0

  name             = "cert-manager"
  repository       = "https://charts.jetstack.io"
  chart            = "cert-manager"
  version          = "1.14.0"
  namespace        = "cert-manager"
  create_namespace = true

  set {
    name  = "installCRDs"
    value = "true"
  }
}

# ingress-nginx — Ingress controller
resource "helm_release" "ingress_nginx" {
  count = var.install_ingress_nginx ? 1 : 0

  name             = "ingress-nginx"
  repository       = "https://kubernetes.github.io/ingress-nginx"
  chart            = "ingress-nginx"
  version          = "4.9.0"
  namespace        = "ingress-nginx"
  create_namespace = true
}
