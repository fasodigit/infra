# SPDX-License-Identifier: AGPL-3.0-only
output "cluster_id" {
  description = "OVH Managed Kubernetes cluster ID"
  value       = ovh_cloud_project_kube.faso_kube.id
}

output "cluster_name" {
  description = "Kubernetes cluster name"
  value       = ovh_cloud_project_kube.faso_kube.name
}

output "cluster_api_url" {
  description = "Kubernetes API server URL"
  value       = ovh_cloud_project_kube.faso_kube.url
  sensitive   = true
}

output "cluster_version" {
  description = "Kubernetes version running on the cluster"
  value       = ovh_cloud_project_kube.faso_kube.version
}

output "node_pool_id" {
  description = "Worker node pool ID"
  value       = ovh_cloud_project_kube_nodepool.workers.id
}

output "kubeconfig" {
  description = "Kubeconfig to access the cluster (store in Vault)"
  value       = ovh_cloud_project_kube.faso_kube.kubeconfig
  sensitive   = true
}
