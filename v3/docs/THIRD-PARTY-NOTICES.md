# Third-party notices — Seal Tools v3.0

Seal Tools bundles the components below. Each keeps its own licence; nothing here changes those
terms. This file ships in the zip beside the guides and should be kept with the application.

## Microsoft DirectML (`DirectML.dll`)

`DirectML.dll` is Microsoft Corporation's DirectX Machine Learning runtime. It is not built or
modified by Seal Tools; it is redistributed unmodified as part of this Windows application.

The Microsoft Software License Terms for Microsoft DirectX Machine Learning permit that: clause 1(a)
grants the right to "copy and distribute the software (i.e. make available for third parties) in
applications and services you develop in the build with Machine Learning tools and frameworks, and/or
games that run on Windows and Xbox", solely for use on Windows and Xbox. The remaining clauses apply
(keep Microsoft's notices, do not reverse engineer or modify the software, do not use it unlawfully or
to propagate malware, comply with export law). The copy shipped here comes from the ONNX Runtime
2.0.0-rc.15 DirectML static distribution and reports FileVersion
`1.15.4+241025-1615.1.dml-1.15.fac7597` (ProductName "DirectML Redistributable Library"). The full
terms as published with the package of that version are at:

- https://feed.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License

DirectML is a trademark of Microsoft Corporation. Seal Tools is not affiliated with or endorsed by
Microsoft; the runtime is used through ONNX Runtime's DirectML execution provider.

## ONNX Runtime

Seal Tools links ONNX Runtime statically (`ort` crate, MIT). ONNX Runtime is:

```
MIT License

Copyright (c) Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## PP-OCRv4 models (`models/*.onnx`)

The detection, recognition and classification models are PaddleOCR's PP-OCRv4 models, converted for
RapidOCR and redistributed under the **Apache License 2.0**. Attribution: PaddleOCR (Apache-2.0),
RapidOCR (Apache-2.0). The models are not modified by Seal Tools; the character dictionary is
extracted from RapidOCRSharp's packaged metadata.

## Flutter and Dart

The shell is a Flutter Windows application. Flutter's own bundled notice file travels with the app at
`data/flutter_assets/NOTICES.Z` (Flutter and the Dart SDK are BSD-3-Clause; bundled plugins keep their
own licences there).

## Rust crates

The core is Rust; its dependencies are licensed MIT or Apache-2.0 (the `ort` stack, `windows-sys`,
`serde`, `sega`-style utilities). The authoritative per-crate licences are in each crate's manifest in
the repository's `Cargo.lock`/`Cargo.toml` metadata. No third-party Rust code is modified.

## Fonts and icons

Material Icons and the Cupertino icon font ship inside the Flutter assets and are covered by Flutter's
NOTICES file above.
