# Chaos Mesh — Installation Guide

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- sovereignty=true -->

## Prérequis

- Kubernetes >= 1.27
- Helm >= 3.10
- `kubectl` configuré sur le cluster cible (FASO DIGITALISATION)
- Namespace `faso` existant

## Installation via Helm

```bash
# 1. Ajouter le repo Chaos Mesh
helm repo add chaos-mesh https://charts.chaos-mesh.org
helm repo update

# 2. Créer le namespace dédié
kubectl create namespace chaos-mesh

# 3. Installer Chaos Mesh
helm install chaos-mesh chaos-mesh/chaos-mesh \
  --namespace chaos-mesh \
  --version 2.6.3 \
  --set controllerManager.replicaCount=1 \
  --set chaosDaemon.runtime=containerd \
  --set chaosDaemon.socketPath=/run/containerd/containerd.sock \
  --set dashboard.create=true \
  --set dashboard.service.type=ClusterIP \
  --wait

# 4. Vérifier l'installation
kubectl get pods -n chaos-mesh
```

## Permissions RBAC (namespace faso)

```bash
# Appliquer les rôles nécessaires pour que Chaos Mesh agisse sur le namespace faso
kubectl apply -f rbac.yaml
```

Exemple `rbac.yaml` :

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: chaos-mesh-role
  namespace: faso
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["get", "list", "watch", "delete"]
  - apiGroups: ["chaos-mesh.org"]
    resources: ["*"]
    verbs: ["*"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: chaos-mesh-binding
  namespace: faso
subjects:
  - kind: ServiceAccount
    name: chaos-controller-manager
    namespace: chaos-mesh
roleRef:
  kind: Role
  name: chaos-mesh-role
  apiGroup: rbac.authorization.k8s.io
```

## Accès au Dashboard

```bash
kubectl port-forward -n chaos-mesh svc/chaos-dashboard 2333:2333
# Ouvrir http://localhost:2333
```

## Désinstallation

```bash
helm uninstall chaos-mesh -n chaos-mesh
kubectl delete namespace chaos-mesh
kubectl delete crd $(kubectl get crd | grep chaos-mesh.org | awk '{print $1}')
```

## Ressources

- Documentation officielle : https://chaos-mesh.org/docs/
- CRD Reference : https://chaos-mesh.org/docs/simulate-pod-chaos-on-kubernetes/
