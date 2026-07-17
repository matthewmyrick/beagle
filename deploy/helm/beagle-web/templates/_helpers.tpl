{{/* Chart name, overridable. */}}
{{- define "beagle-web.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/* Fully-qualified app name. */}}
{{- define "beagle-web.fullname" -}}
{{- printf "%s-%s" .Release.Name (include "beagle-web.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/* Common labels. */}}
{{- define "beagle-web.labels" -}}
app.kubernetes.io/name: {{ include "beagle-web.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version }}
{{- end -}}

{{/* Selector labels for the web deployment. */}}
{{- define "beagle-web.selectorLabels" -}}
app.kubernetes.io/name: {{ include "beagle-web.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: web
{{- end -}}

{{/* Selector labels for the auth proxy. */}}
{{- define "beagle-web.authSelectorLabels" -}}
app.kubernetes.io/name: {{ include "beagle-web.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: auth
{{- end -}}

{{/* The image ref, defaulting the tag to appVersion. */}}
{{- define "beagle-web.image" -}}
{{- printf "%s:%s" .Values.image.repository (.Values.image.tag | default .Chart.AppVersion) -}}
{{- end -}}

{{/* Secret name holding the auth secrets. */}}
{{- define "beagle-web.authSecretName" -}}
{{- if .Values.auth.existingSecret -}}
{{- .Values.auth.existingSecret -}}
{{- else -}}
{{- printf "%s-auth" (include "beagle-web.fullname" .) -}}
{{- end -}}
{{- end -}}
