# SPDX-License-Identifier: AGPL-3.0-only
# Terraform module — OVH Managed Kubernetes (3 nodes B3-8)
# Design only — no real cloud provisioning
terraform {
  required_version = ">= 1.7.0"
  required_providers {
    ovh = {
      source  = "ovh/ovh"
      version = ">= 0.46.0"
    }
  }
  backend "s3" {
    # OVH Object Storage (Swift/S3-compatible) for remote state
    bucket                      = "faso-terraform-state"
    key                         = "ovh-kube/terraform.tfstate"
    region                      = "eu-west-par"
    endpoint                    = "https://s3.gra.io.cloud.ovh.net"
    skip_credentials_validation = true
    skip_region_validation      = true
    skip_metadata_api_check     = true
    skip_requesting_account_id  = true
  }
}

provider "ovh" {
  endpoint           = "ovh-eu"
  application_key    = var.ovh_application_key
  application_secret = var.ovh_application_secret
  consumer_key       = var.ovh_consumer_key
}

# OVH Managed Kubernetes cluster
resource "ovh_cloud_project_kube" "faso_kube" {
  service_name = var.ovh_service_name
  name         = "faso-kube-${var.environment}"
  region       = var.region

  version = var.kubernetes_version

  private_network_id = ovh_cloud_project_network_private.faso_vpc.id

  private_network_configuration {
    default_vrack_gateway              = ""
    private_network_routing_as_default = true
  }

  update_policy = "MINIMAL_DOWNTIME"

  customization_apiserver {
    admissionplugins {
      enabled  = ["NodeRestriction"]
      disabled = []
    }
  }
}

# Node pool — 3x B3-8 (8 vCPU, 32 GB RAM)
resource "ovh_cloud_project_kube_nodepool" "workers" {
  service_name  = var.ovh_service_name
  kube_id       = ovh_cloud_project_kube.faso_kube.id
  name          = "workers-${var.environment}"
  flavor_name   = "b3-8"  # 8 vCPU / 32 GB RAM
  desired_nodes = 3
  min_nodes     = 3
  max_nodes     = 6
  autoscale     = true
  monthly_billed = false

  template {
    metadata {
      labels = {
        "topology.kubernetes.io/region" = var.region
        "faso.gov.bf/environment"       = var.environment
        "faso.gov.bf/tier"              = "worker"
      }
      annotations = {}
    }
    spec {
      unschedulable = false
      taints        = []
    }
  }
}

# Private VPC network
resource "ovh_cloud_project_network_private" "faso_vpc" {
  service_name = var.ovh_service_name
  name         = "faso-vpc-${var.environment}"
  regions      = [var.region]
  vlan_id      = 100
}

resource "ovh_cloud_project_network_private_subnet" "faso_subnet" {
  service_name = var.ovh_service_name
  network_id   = ovh_cloud_project_network_private.faso_vpc.id
  region       = var.region
  start        = "10.0.0.1"
  end          = "10.0.255.254"
  network      = "10.0.0.0/16"
  dhcp         = true
  no_gateway   = false
}
