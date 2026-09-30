# NUR OS — Licensing and Dependency Policy

Status: **PROPOSED policy.** This document is engineering guidance, **not legal advice**. All
categorisations must be confirmed by qualified counsel before commercial distribution.

## 1. NUR's own code

The license for NUR-authored code is **not decided** and requires approval by ADM Tech Holding
MChJ ([RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md)). Options:

| Option | Effect |
| --- | --- |
| Proprietary (all rights reserved) for everything | Maximum control; limits community contribution and trust in a security-sensitive OS |
| Permissive (Apache-2.0 or MIT/Apache dual) for platform + SDK, proprietary for selected services (Store backend, cloud, AI services) | Encourages ecosystem adoption; Apache-2.0 gives an explicit patent grant |
| Copyleft (GPL-3.0 / MPL-2.0) for OS components | Keeps derivatives open; MPL is file-level and more commercial-friendly |

Engineering recommendation: **Apache-2.0 for platform code, SDK, and tooling; separate decision
for server-side services**. Until decided, no LICENSE file is added and all rights are reserved.

Every NUR source file will carry an SPDX header once the license is chosen (REUSE specification).

## 2. Contributor terms

DCO (Developer Certificate of Origin, `Signed-off-by`) vs CLA (Contributor License Agreement).
A CLA is typically needed if ADM Tech wants the right to relicense or dual-license contributions
commercially. **Decision required.**

## 3. Third-party license categories

| Category | Licenses (SPDX) | Policy |
| --- | --- | --- |
| **A — Allowed** | MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0, ISC, Zlib, BSL-1.0, Unicode-3.0 / Unicode-DFS-2016, CC0-1.0, 0BSD, OFL-1.1 (fonts) | Allowed in any component. Preserve notices; Apache-2.0 requires NOTICE propagation |
| **B — Weak copyleft, allowed with review** | MPL-2.0, LGPL-2.1-or-later, LGPL-3.0-or-later, EPL-2.0 | Allowed. Linking rules must be respected: LGPL libraries dynamically linked (or relinkable object files provided) in non-LGPL components; modified MPL files published |
| **C — Strong copyleft, allowed as separate programs** | GPL-2.0-only, GPL-2.0-or-later, GPL-3.0-or-later | Allowed as independent programs in the OS image (e.g. kernel, many system tools). **Must not be linked into** NUR components whose license is not GPL-compatible. Source for shipped binaries must be offered per license terms. GPL-3.0 installation-information ("anti-tivoisation") obligations must be analysed for Secure Boot devices — see §5 |
| **D — Network copyleft, legal approval required** | AGPL-3.0 | Not used in NUR client or service code without legal approval; server-side use triggers source-offer obligations to network users |
| **E — Prohibited** | SSPL-1.0, BUSL-1.1 (until converted), Commons Clause, Elastic License, "non-commercial" (e.g. CC-BY-NC), JSON License ("shall be used for Good, not Evil"), no license / unknown, proprietary without a signed agreement | Not allowed |
| **F — Content / data** | CC-BY-4.0, CC-BY-SA-4.0, ODbL, model-weight licenses | Case-by-case; attribution tracking; AI model licenses reviewed for use restrictions |

Dual-licensed dependencies are used under the most permissive option available and recorded.

## 4. Obligations summary

| License | Key obligations when distributing binaries |
| --- | --- |
| MIT / BSD / ISC | Include copyright and license text |
| Apache-2.0 | Include license + NOTICE; state changes in modified files; patent termination clause |
| MPL-2.0 | Source of MPL-covered files (incl. modifications) available; notices preserved |
| LGPL-2.1 / 3.0 | Source of the library (incl. modifications); allow user to relink/replace library; LGPL-3.0 also requires installation information for "User Products" |
| GPL-2.0 | Complete corresponding source (incl. build scripts) with offer or alongside binaries |
| GPL-3.0 | As GPL-2.0 + installation information for User Products + patent license terms |
| AGPL-3.0 | As GPL-3.0 + source offer to network users of modified versions |
| OFL-1.1 | Fonts may be bundled; reserved font names cannot be used for modified versions |

Release pipeline must produce: SBOM with license per component, a combined third-party notices
file shipped in the image (`/usr/share/doc/nur/THIRD_PARTY_NOTICES`), and a source archive /
source-offer mechanism for all copyleft components.

## 5. Specific questions requiring legal review

1. **GPL-3.0 / LGPL-3.0 installation information** vs Secure Boot: we must ensure users can
   install modified versions (e.g. by enrolling their own keys / using MOK / disabling Secure
   Boot through firmware). Documented user key enrolment is the proposed answer.
2. **NVIDIA proprietary userspace** redistribution terms and kernel-module signing.
3. **Media codecs** (H.264/HEVC/AAC and others): patent licensing for distribution in target
   markets; option of shipping only royalty-free codecs by default with user-installed extras.
4. **Fonts** bundled in documents and UI.
5. **Office engine** (see [OFFICE_STRATEGY.md §5](OFFICE_STRATEGY.md#5-licensing-review-checklist-must-be-completed-before-any-redistribution)).
6. **AI model weights** licensing and acceptable-use restrictions.
7. **Trademarks**: "NUR", "NUR OS" clearance in Uzbekistan and target export markets; removal of
   upstream trademarks (Debian, Firefox, LibreOffice) where required by their policies. Shipping
   unmodified Mozilla Firefox builds keeps the trademark; modifying requires rebranding.
8. **Base distribution**: Debian's trademark policy for derivatives ("based on Debian" wording).
9. **Export control** of cryptographic software in target markets.

## 6. Adoption process for a new dependency

1. Check this policy's category. E → stop.
2. Record in the dependency register (PR description now; `cargo-deny` config + SBOM later):
   name, version, license (verified from source, not metadata alone), purpose, alternatives,
   maintenance health (recent releases, maintainers, security response), transitive dependency
   count.
3. Category B/C/D: note linking model and obligations; D requires legal approval.
4. Security review for dependencies used in privileged contexts.

## 7. Never

- Never copy proprietary Windows, macOS, or other vendors' code, assets, icons, fonts, sounds, or
  documentation.
- Never ship Microsoft redistributables with Wine without a valid license.
- Never remove license notices or copyright headers from third-party code.
