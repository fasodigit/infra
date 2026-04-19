# SPDX-License-Identifier: AGPL-3.0-only
# Post-cluster bootstrap — namespace labels, PodSecurityStandards, Flux install
# Applied via Terraform null_resource after cluster is ready

locals {
  namespaces = {
    "kaya"         = { tier = "data",    pss = "restricted" }
    "gateway"      = { tier = "gateway", pss = "restricted" }
    "apps"         = { tier = "app",     pss = "restricted" }
    "ory-stack"    = { tier = "security",pss = "restricted" }
    "observability"= { tier = "infra",   pss = "baseline"   }
    "spire"        = { tier = "security",pss = "restricted" }
    "flux-system"  = { tier = "gitops",  pss = "restricted" }
    "argo-rollouts"= { tier = "gitops",  pss = "restricted" }
    "cert-manager" = { tier = "infra",   pss = "baseline"   }
    "ingress-nginx"= { tier = "infra",   pss = "baseline"   }
    "external-secrets" = { tier = "security", pss = "restricted" }
    "vault"        = { tier = "security",pss = "restricted" }
  }
}

resource "null_resource" "bootstrap_namespaces" {
  depends_on = [ovh_cloud_project_kube_nodepool.workers]

  triggers = {
    cluster_id = ovh_cloud_project_kube.faso_kube.id
  }

  provisioner "local-exec" {
    command = <<-EOT
      export KUBECONFIG=<(echo ${base64encode(ovh_cloud_project_kube.faso_kube.kubeconfig)})
      %{for ns, labels in local.namespaces~}
      kubectl create namespace ${ns} --dry-run=client -o yaml | kubectl apply -f -
      kubectl label namespace ${ns} \
        faso.gov.bf/tier=${labels.tier} \
        pod-security.kubernetes.io/enforce=${labels.pss} \
        pod-security.kubernetes.io/enforce-version=latest \
        pod-security.kubernetes.io/warn=${labels.pss} \
        pod-security.kubernetes.io/audit=${labels.pss} \
        --overwrite
      %{endfor~}
    EOT
  }
}

resource "null_resource" "bootstrap_flux" {
  depends_on = [null_resource.bootstrap_namespaces]

  triggers = {
    cluster_id = ovh_cloud_project_kube.faso_kube.id
  }

  provisioner "local-exec" {
    command = <<-EOT
      flux bootstrap github \
        --owner=fasodigitalisation \
        --repository=infra \
        --branch=main \
        --path=deploy/gitops/flux-system \
        --personal \
        --namespace=flux-system
    EOT
  }
}
