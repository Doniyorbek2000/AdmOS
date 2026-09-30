# NUR OS — Kernel Configuration Policy

Status: **Phase 1 — implemented as a build-time audit.** The machine-readable policy is
[`image/kernel/policy.json`](../image/kernel/policy.json); `image/mkosi.finalize` checks the
kernel's build configuration against it for every image and writes
`nur_<version>.kernel-audit.json`, summarised in the build record. A missing **required**
option aborts the build.

## 1. Kernel source (ADR-0001)

| Base | Kernel | Series | Status |
| --- | --- | --- | --- |
| Debian trixie (canonical, D1) | `linux-image-amd64` from Debian | 6.12 LTS | Configured; **not built in the Phase 1 sandbox** (Debian mirrors blocked by the environment's egress policy) |
| Ubuntu noble (environment substitute) | `linux-image-virtual` (6.8.0-x-generic) | Ubuntu-maintained 6.8 (not an upstream LTS) | Built and booted in QEMU; used only to exercise NUR layers |

Phase 1 uses the base distribution's kernel build unmodified. ADR-0001's target of a NUR-built
kernel with a NUR configuration is **deferred** (technical debt TD8): it requires a kernel build
and module-signing pipeline, which should be built once the canonical Debian path is running in
CI. No kernel patches are carried.

## 2. Required options

| Area | Options | Why |
| --- | --- | --- |
| Boot | `EFI_STUB` | UKIs are booted by systemd-stub / systemd-boot |
| Integrity | `BLK_DEV_DM`, `DM_VERITY` | `/usr` is verified block-by-block |
| Encryption | `DM_CRYPT`, `CRYPTO_XTS` | LUKS2 state partition (AES-XTS) |
| File systems | `EROFS_FS` (`/usr`), `EXT4_FS` (state) | Image and state formats |
| TPM | `TCG_TPM`, `TCG_TIS`, `TCG_CRB` | TPM2-bound unlock, measured boot |
| Lockdown | `SECURITY_LOCKDOWN_LSM` | Integrity lockdown under Secure Boot |
| Modules | `MODULE_SIG` | Signed kernel modules |
| Sandboxing | `SECCOMP`, `SECCOMP_FILTER`, `SECURITY_LANDLOCK`, `SECURITY_YAMA` | Service and app confinement |
| Namespaces / cgroups | `NAMESPACES`, `USER_NS`, `PID_NS`, `NET_NS`, `CGROUPS`, `MEMCG` | systemd service hardening, Flatpak (later), resource control |
| Hardening | `RANDOMIZE_BASE`, `STACKPROTECTOR_STRONG`, `HARDENED_USERCOPY`, `STRICT_DEVMEM`, `KEXEC_SIG` | Exploit mitigation; no unsigned kexec bypass of Secure Boot |

Virtualization support needed for development (virtio-blk, virtio console, TPM TIS) is provided by
the distribution kernels; hardware enablement is Phase 4.

## 3. Recommended options (reported, not yet enforced)

`MODULE_SIG_FORCE` (Debian/Ubuntu instead rely on lockdown to refuse unsigned modules when Secure
Boot is on), `LOCK_DOWN_KERNEL_FORCE_INTEGRITY`, `INIT_ON_ALLOC_DEFAULT_ON`,
`BPF_UNPRIV_DEFAULT_OFF`, `DM_VERITY_VERIFY_ROOTHASH_SIG`, `IO_STRICT_DEVMEM`, `IMA`, `EVM`.
These become required when NUR builds its own kernel.

## 4. Security mitigations

CPU vulnerability mitigations stay at the kernel defaults. They are never disabled for
performance (`mitigations=off` is forbidden in NUR kernel command lines).

## 5. IMA/EVM

**Research.** dm-verity already protects the whole of `/usr`, and the mutable state is encrypted.
IMA appraisal of the state partition would add runtime file integrity for `/etc` and `/var` but
brings policy-management and performance costs. It will be evaluated with the MAC decision
(ADR-0015). IMA/EVM are compiled into both base kernels but not configured by NUR.

## 6. Development vs release configuration

| Aspect | Development (Phase 1) | Release (future) |
| --- | --- | --- |
| Kernel | Base distribution build | NUR build from upstream LTS, NUR config, NUR module-signing key |
| `DEBUG_FS`, `KALLSYMS_ALL`, `DEVMEM` | Present (reported under `forbidden_in_release`) | Off (with `DEVMEM`, `STRICT_DEVMEM` and `IO_STRICT_DEVMEM` instead) |
| Kernel command line | `console=ttyS0`, `panic=10`, optional journal-to-console for debugging | No debug options; the command line is inside the signed UKI in both cases |
| Serial root autologin | Yes (test harness; dev profile only) | Never |
