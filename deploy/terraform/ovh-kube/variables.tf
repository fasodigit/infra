# SPDX-License-Identifier: AGPL-3.0-only
variable "ovh_application_key" {
  description = "OVH API application key"
  type        = string
  sensitive   = true
}

variable "ovh_application_secret" {
  description = "OVH API application secret"
  type        = string
  sensitive   = true
}

variable "ovh_consumer_key" {
  description = "OVH API consumer key"
  type        = string
  sensitive   = true
}

variable "ovh_service_name" {
  description = "OVH Cloud project service name (UUID)"
  type        = string
}

variable "environment" {
  description = "Deployment environment (prod, staging)"
  type        = string
  default     = "prod"

  validation {
    condition     = contains(["prod", "staging", "dev"], var.environment)
    error_message = "environment must be prod, staging, or dev."
  }
}

variable "region" {
  description = "OVH region (EU sovereignty)"
  type        = string
  default     = "GRA11"  # Gravelines, France — EU data sovereignty

  validation {
    condition     = can(regex("^(GRA|SBG|WAW|DE)", var.region))
    error_message = "Only EU OVH regions are allowed for data sovereignty."
  }
}

variable "kubernetes_version" {
  description = "Kubernetes version"
  type        = string
  default     = "1.31"
}
