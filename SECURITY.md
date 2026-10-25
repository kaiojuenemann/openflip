# Security policy

## Supported versions

Security fixes go into the latest release. openflip is young; there are no
long-term support branches yet.

## Reporting a vulnerability

Please do **not** open a public issue for a security problem. Report it
privately through GitHub instead: on the repository page, open **Security →
Report a vulnerability**
(<https://github.com/kaiojuenemann/openflip/security/advisories/new>).

Please describe what you found, how to reproduce it and which version and
operating system you used. You will get an answer as soon as possible. Once
a fix is released, the advisory is published with credit to you, unless you
prefer otherwise.

## Verifying downloads

Every release lists a `SHA256SUMS` file. Check a download against it:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing   # macOS
sha256sum -c SHA256SUMS --ignore-missing       # Linux
```

On Windows: `Get-FileHash openflip-<version>-windows.zip` in PowerShell, and
compare with the line in `SHA256SUMS`.

Releases published while the repository is public also carry a signed build
provenance attestation, which shows that a package was built by this
repository's CI:

```sh
gh attestation verify openflip-<version>-macos.zip --repo kaiojuenemann/openflip
```

The macOS bundles are not notarized and the Windows screensaver is not
signed yet (see the roadmap), which is why the install guides explain how to
open them anyway. Only do that for files that pass the checks above.
