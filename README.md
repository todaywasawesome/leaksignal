<p align="center">
  <img src="assets/logo-black-red.png?sanitize=true#gh-light-mode-only" width="800">
  <img src="assets/logo-black-red.png?sanitize=true#gh-dark-mode-only" width="800">
</p>

<h4 align="center">
  <a href="https://www.leaksignal.com">Website</a> |
  <a href="https://blog.leaksignal.com">Blog</a> |
  <a href="https://docs.leaksignal.com">Documentation</a> |
  <a href="https://docs.leaksignal.com/faq/">FAQ</a> |
  <a href="https://slack.leaksignal.com">Slack</a>
</h4>

<p align="center">
  <a href="https://github.com/leaksignal/leaksignal/blob/master/LICENSE"><img src="https://img.shields.io/hexpm/l/plug" alt="License"></a>
</p>

<p align="center">🔍 There’s all kinds of sensitive data flowing through my services, but I don’t know which ones or what data. 🤷</p>

LeakSignal provides observability by generating metrics (or [statistics](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/observability/statistics#arch-overview-statistics)) for sensitive data that is contained in request/response content. LeakSignal metrics can be consumed by Prometheus, pushed as OpenTelemetry, or collected in a centralized dashboard.

### Features
* Fast, inline Layer 7 request/response analysis.
* Easy to configure rules ("L7 policy") for detecting and analyzing sensitive data (e.g. PII) leakage.
  * Detect part numbers, account numbers, patient info, grades, dates, email addresses, large arrays, etc. The available <a href="somewhere">ruleset</a> is constantly evolving.
* Cloud dashboard with policy editor, monitoring, and alerting. 
* Analysis metrics can be exposed via Envoy and thus reflected wherever Envoy metrics are configured to land (OpenTelemetry, Prometheus, etc.)

### Installation

LeakSignal installs in moments as a WASM filter for Envoy, Istio, or any proxy/API gateway that supports Proxy-WASM. See Getting Started below.

### Overview
LeakSignal detects sensitive data within mesh traffic. Analysis and processing of traffic is handled inline, allowing engineers to understand sensitive data leaks without sending, storing or viewing the sensitive data.
<p align="center">
  <img style="max-width:75%" src="assets/mesh-overview2.png">
</p>

### LeakSignal SENTRY
LeakSignal establishes a framework and delivery mechanism for composable traffic analysis functions within a WASM VM. Sentry is the bytecode that allows for sensitive data analysis across request and response traffic in real-time.
<p align="center">
  <img style="max-width:75%" src="assets/filter-overview2.png">
</p>
The following functionality can be enabled through the Layer7 Policy:

* Sensitive Data Observability
* Data Access by IP or Token
* Exfiltration Mitigation
* Data Access Auditing
* Prometheus and OTEL metrics
* Dashboard visualization (histogram, heatmap) and alerting via SMS or email

### LeakSignal COMMAND
LeakSignal Command (the cloud dashboard) provides visibility of data types and sends you SMS or email alerts when abnormal or unauthorized data access occurs.
<p align="center">
  <img style="max-width:75%" src="assets/command-overview1.png">
</p>

### Implementation
Built with Rust and deployed as WebAssembly, LeakSignal natively runs on proxies and API Gateways supporting [Proxy-WASM](https://github.com/proxy-wasm/spec). The current implementation is tested with Envoy, which is the underlying data management plane in most service mesh offerings.

LeakSignal analysis can be setup in the following modes:
* All metrics and configuration stay local in your environment
* All metrics and configuration go to LeakSignal COMMAND.
  * Sensitive data is never stored or transferred to COMMAND. Only metrics describing the analysis along with policy configuration are stored. Feel free to examine this part of the codebase (insert link to line #) where we form the telemetry.
* (@Issac there might be a better way to word this? ^^^)

## Getting Started with a Demo

If you're looking to kick the tires with a demo setup, you have 2 options:
1. [Simple Envoy Ingress controller for K8s cluster](https://github.com/leaksignal/testing-environments).
2. [Google's Online Boutique microservices demo for Istio](https://github.com/leaksignal/testing-environments/istio).

  
## Getting Started with Existing Setup 
If you already have an environment up and running (Standalone Envoy, K8s, or Istio) where you'd like to install LeakSignal, use the following quick starts.

### Envoy Quickstart
Docker commands to run an Envoy proxy with LeakSignal installed. 

1. [Register for an account](https://app.leaksignal.com/register), create a deployment and get your API key.
2. Your API key is located in [@max @rett]
3. Create a simple barebones deployment by vlicking "Deployments" in the left hand nav and select "Create Deployment"
4. Replace YOUR-API-KEY and YOUR-DEPLOYMENT-NAME below with the values in LeakSignal Command.

```
FROM envoyproxy/envoy-dev:0b1c5aca39b8c2320501ce4b94fe34f2ad5808aa
RUN curl -O https://raw.githubusercontent.com/leaksignal/proxy-wasm/main/testing/envoy.yaml
RUN sed -i 's/api_key_placeholder/YOUR-API-KEY/g' envoy.yaml
RUN sed -i 's/deployment_name_placeholder/YOUR-DEPLOYMENT-NAME/g' envoy.yaml
COPY ./envoy.yaml /etc/envoy.yaml
RUN chmod go+r /etc/envoy.yaml
CMD ["/usr/local/bin/envoy", "-c", "/etc/envoy.yaml"]
```
> * Go to Deployments -> YOUR-DEPLOYMENT-NAME and learn more about the L7 Policy that is currently running.
> * [View metrics in COMMAND](#view-metrics-command)


### Envoy-Local Quickstart (no cloud connection)
Docker commands to run an Envoy proxy with LeakSignal installed. This configuration runs LeakSignal in "local" mode where metrics are only exported in the running Envoy instance. Additionally, the LeakSignal L7 Policy is contained in the yaml configuration. LeakSignal API Key and deployment name are not needed.
```
FROM envoyproxy/envoy-dev:0b1c5aca39b8c2320501ce4b94fe34f2ad5808aa
RUN curl -O https://raw.githubusercontent.com/leaksignal/proxy-wasm/main/testing/envoy_standalone.yaml
COPY ./envoy.yaml /etc/envoy.yaml
RUN chmod go+r /etc/envoy.yaml
CMD ["/usr/local/bin/envoy", "-c", "/etc/envoy.yaml"]
```
> * [Verify everything is setup correctly](#verify-proper-setup).
> * Test and configure L7 Policy for your environment
> * [View prometheus metrics in grafana](#view-metrics-prometheus)

Use the [demo environment](https://github.com/leaksignal/testing-environments) to see a working example. Your sensitive data labels and counts will be exported as Envoy metrics. 


### Istio
Install LeakSignal across all Istio sidecar proxies with the following:

1. [Register for an account](https://app.leaksignal.com/register), create a deployment and get your API key.
2. Your API key is located in [@max @rett]
3. Create a simple barebones deployment by vlicking "Deployments" in the left hand nav and select "Create Deployment"
4. Replace YOUR-API-KEY and YOUR-DEPLOYMENT-NAME below with the values in LeakSignal Command.

```
#set Istio to preview mode
istioctl install --set profile=preview

# Apply the following leaksignal.yaml to deploy the filter
curl https://github.com/leaksignal/istio/leaksignal.yaml | \
‍sed -i 's/api_key_placeholder/YOUR-API-KEY/g' | \
sed -i 's/deployment_name_placeholder/YOUR-DEPLOYMENT-NAME/g' | kubectl apply -f -

#restart all the pods
kubectl delete --all pod
```
> Go to Deployments -> YOUR-DEPLOYMENT-NAME and learn more about the L7 Policy that is currently running.

### Istio-Local (no cloud connection)
Install LeakSignal across all Istio sidecar proxies with the following. Metrics will be exported in Envoy and L7 Policy is contained in the yaml configuration. LeakSignal API Key and deployment name are not needed.
```
#set Istio to preview mode
istioctl install --set profile=preview

# Apply the following leaksignal.yaml to deploy the filter
curl https://github.com/leaksignal/istio/leaksignal.yaml | kubectl apply -f -

#restart all the pods
kubectl delete --all pod
```
> * [Verify everything is setup correctly](#verify-proper-setup).
> * Test and configure L7 Policy for your environment

### Verify Proper Setup
After you've installed the LeakSignal filter, you can check the logs to see how things are running:

For Envoy standalone run:
```
tail -f /var/log/envoy.log
```
For Kubernetes run:
```
kubectl get pods
#find the envoy pod and use it below
kubectl logs -f [envoy podname]
```
For Istio run:
```
kubectl -n istio-system get pods
kubectl -n istio-system logs istio-ingressgateway-abc123
```
In all cases you should see messsages with "leaksignal" in the logs. Use those to understand if things are setup correctly. Note that you may see messages like `createWasm: failed to load (in progress) from https://ingestion.app...` if loading the wasm file remotely. This is a known issue and the wasm filter is functioning properly.  


### View Metrics (Prometheus)
Next we'll check Prometheus to ensure LeakSignal metrics are ingested. (If you don't have or want to use Prometheus skip to the next step)
Here's an example from the demo in where grafana displays LeakSignal metrics from prometheus:
![](/assets/sd_per_min.png)

### View Metrics (COMMAND)
Once you login to LeakSignal COMMAND, you'll see the Sensitive Data Overview as the default screen:
<img src="assets/dashboard.png" width="550">
- Overview - 
- Drilling down on data
- Performance metrics
- Visualizing a leak


### Test and configure L7 Policy
After you've verified that the filter is running, you can configure the policy to check for specific sensitive data types or patterns. For examples of preconfigured and performance tested policies, see [LeakSignal Policies](#policies)

All regex is standard PCRE.

@max do we have a yaml schema? (seems like there's one specific for command and one for local e.g. alerts)

- Match rules - how to write ( nd optimize)
- Creating Alerts
- Sensitive Data Access Limits

## Troubleshooting
- Viewing logs across services

## Community / How to Contribute
- Slack Public - Regex Channel, Policy Channel, K8s
- Code contribution guidelines

## Commercial support
- Leaksignal, Inc offers support and self-hosted versions of the cloud dashboard. Contact sales@leaksignal.com.

## License 
- Apache2 


