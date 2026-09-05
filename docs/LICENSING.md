# RustRDP licensing

RustRDP's original code, documentation and artwork are licensed under the
GNU General Public License, version 3 only (`GPL-3.0-only`). The complete,
unmodified license is in [LICENSE](../LICENSE). Copyright and project attribution
are in [NOTICE](../NOTICE).

## Use and modification

Personal and business use are permitted without a license fee. You may inspect,
modify and run the software. Modifications used privately or internally within
an organization do not need to be published merely because they were made.

## Sharing and selling

GPLv3 permits redistribution, renaming, commercial use and selling copies.
RustRDP does not impose a noncommercial or no-resale restriction.

When distributing copies or derivatives, comply with GPLv3, including:

- preserve the required copyright, license and warranty notices;
- include the GPLv3 license;
- mark modified versions with prominent notices of changes and their dates;
- license distributed derivative works as a whole under GPLv3;
- when distributing binaries, provide corresponding source through a method
  allowed by section 6, including the necessary build and installation material.

These obligations preserve the original notices and recipients' source rights.
They do not require an advertising credit or a public GitHub repository. Source
must be provided as required by the chosen GPL distribution method; recipients
may redistribute it. Separate, independent works are not automatically GPL
derivatives just because they are distributed alongside RustRDP.

## Third-party work

The project license does not replace upstream licenses or claim authorship of
upstream work. See [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) and
[assets/LICENSES.md](../assets/LICENSES.md).

In particular, the vendored eframe code remains MIT OR Apache-2.0, and bundled
fonts retain their respective licenses. Other dependencies may carry their own
source-sharing and notice requirements, including the MPL-2.0 dependency
`option-ext`. Preserve those rights and satisfy them when distributing a build.

## Release packaging

Before distributing a binary, review the exact dependency graph in Cargo.lock,
collect the applicable license texts and copyright notices, and satisfy all
source-availability requirements. Include RustRDP's LICENSE, NOTICE and
THIRD_PARTY_NOTICES.md with the distribution. For downloads, provide clear access
to the matching corresponding source alongside the binary, as required by
GPLv3 section 6(d). A source link must cover the actual build, including patches
and required dependency source, not merely point to a changing main branch.

The local installer includes the project license and summary notices under
`~/.local/share/rustrdp/legal/`. These summaries are not a complete binary
dependency-license bundle. The full dependency and Qt/KDE runtime compliance
review remains a prerequisite for public binary distribution. FreeRDP and KDE/Qt
runtime packages are currently installed separately, not bundled by RustRDP.

## Future license changes

Rights already granted under GPLv3 cannot be withdrawn from compliant recipients.
A future release under different terms requires the necessary rights to all
included code. Contributors retain their copyrights; their GPL contributions
do not automatically authorize incompatible relicensing. Third-party licenses
continue to apply independently.

This guide summarizes the license; the full GPLv3 text controls.
