# Security Policy

## Supported Versions

The following table describes the version support status for rJMX-Exporter:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

> **Note:** As an early-stage project, we currently support only the latest minor release. Security patches will be applied to the most recent version.

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**Preferred Method: GitHub Security Advisories**

1. Navigate to the [Security Advisories](https://github.com/jsoonworld/rJMX-Exporter/security/advisories) page
2. Click "Report a vulnerability"
3. Fill out the advisory form with details about the vulnerability

**Alternative: Email**

If you are unable to use GitHub Security Advisories, you may report vulnerabilities via email to the project maintainers. Please include `[SECURITY]` in the subject line.

### What to Include in Your Report

To help us understand and address the issue quickly, please include:

- **Description**: A clear description of the vulnerability
- **Impact**: The potential impact and severity of the issue
- **Affected Versions**: Which versions are affected
- **Reproduction Steps**: Detailed steps to reproduce the vulnerability
- **Proof of Concept**: Code snippets, screenshots, or logs demonstrating the issue (if applicable)
- **Suggested Fix**: Any recommendations for remediation (if available)

### Response Timeline

| Action | Timeline |
| ------ | -------- |
| Acknowledgment of report | Within 48 hours |
| Initial assessment | Within 7 days |
| Status update | Every 7 days until resolution |
| Security advisory publication | Upon fix release |

We aim to resolve critical vulnerabilities as quickly as possible. The timeline for a fix depends on the complexity of the issue.

### Disclosure Policy

- Please do not publicly disclose the vulnerability until we have had a chance to address it
- We will coordinate with you on the disclosure timeline
- We will credit reporters in our security advisories (unless you prefer to remain anonymous)

## Security Best Practices

When deploying rJMX-Exporter, follow these best practices to maintain a secure environment:

### Network Configuration

- **Restrict network access**: Bind the exporter to localhost or internal networks only when possible
- **Use firewalls**: Limit access to the metrics endpoint (`/metrics`) to authorized Prometheus servers
- **Enable TLS**: Use a reverse proxy (e.g., nginx, Caddy) to terminate TLS for production deployments

### Access Control

- **Authentication**: Consider placing the exporter behind an authenticating proxy for sensitive environments
- **Least privilege**: Run the exporter with minimal required permissions
- **Container security**: When running in Docker/Kubernetes, use non-root users and read-only file systems

### Configuration Security

- **Protect configuration files**: Ensure YAML configuration files have appropriate file permissions (e.g., `chmod 600`)
- **Validate target URLs**: Only configure trusted Jolokia endpoints as targets
- **Review exposed metrics**: Be aware of what JMX data is being exposed through the exporter

### Monitoring and Logging

- **Enable logging**: Use appropriate log levels to detect suspicious activity
- **Monitor access**: Track access to the metrics endpoint
- **Regular updates**: Keep rJMX-Exporter and its dependencies up to date

## Known Security Considerations

### JMX Data Exposure

rJMX-Exporter collects metrics from JMX endpoints via Jolokia. Be aware of the following:

- **Sensitive data**: JMX can expose sensitive operational data including memory usage, thread information, and application-specific metrics
- **Metric filtering**: Use rules configuration to limit which MBeans are exported
- **Data classification**: Understand what data your JMX endpoints expose before configuring the exporter

### Network Security

- **Jolokia communication**: Traffic between rJMX-Exporter and Jolokia endpoints is over HTTP/HTTPS. Ensure Jolokia is properly secured
- **Prometheus scraping**: The `/metrics` endpoint exposes collected data. Secure this endpoint appropriately
- **Sidecar deployment**: When running as a sidecar, ensure pod-level network policies are in place

### Dependency Security

- We regularly update dependencies to address known vulnerabilities
- We use `cargo audit` to scan for security advisories in dependencies
- Report any concerns about dependencies through our vulnerability reporting process

## Security Updates

Security updates will be announced through:

- GitHub Security Advisories
- Release notes
- The project's changelog

We recommend watching the repository for security notifications and keeping your installation up to date.

---

Thank you for helping keep rJMX-Exporter and its users safe.
