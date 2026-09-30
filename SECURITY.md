# Security Policy

## Supported versions

NUR OS has not published any release. There are no supported versions yet. The support policy
for future releases is defined in [docs/RELEASE_STRATEGY.md](docs/RELEASE_STRATEGY.md).

## Reporting a vulnerability

Do **not** open a public issue for a suspected vulnerability.

Until a dedicated security contact and PGP key are published, report vulnerabilities privately
through GitHub's **"Report a vulnerability"** (private vulnerability reporting) on this
repository. Include:

- affected component and version/commit;
- reproduction steps or proof of concept;
- impact assessment as you understand it;
- whether the issue is already public.

Target handling (to be formalised before the first public preview):

| Step | Target |
| --- | --- |
| Acknowledge report | 3 business days |
| Initial triage and severity | 10 business days |
| Fix or mitigation for Critical/High | Coordinated; default disclosure deadline 90 days |

## Scope

In scope: all code, build definitions, signing and update infrastructure, and documentation in
this repository. Vulnerabilities in upstream projects should also be reported upstream; we will
coordinate where NUR OS ships the affected component.

## Secrets

Never commit credentials, signing keys, tokens, or private keys. If one is committed
accidentally, treat it as compromised: revoke/rotate it first, then remove it from history.
The security architecture is described in [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md).
