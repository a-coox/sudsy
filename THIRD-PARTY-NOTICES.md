# Third-party notices

Sudsy's own code is MIT ([LICENSE](LICENSE)). These files in the repository come from other
projects. Apart from the logo, none of them is part of the released binaries; they are test and benchmark inputs.

* `assets/logo/`: the Soap emoji of [Fluent UI Emoji](https://github.com/microsoft/fluentui-emoji) (commit `1ffb34c`), Copyright (c) Microsoft Corporation, MIT ([assets/logo/LICENSE](assets/logo/LICENSE)). Used as the logo of the extension, site and README.

## MIT-licensed files

Each project below is licensed under the MIT License, whose text is given once at the end, with
the copyright line of that project.

| Files in this repository | Source | Copyright |
| --- | --- | --- |
| `bench/inputs/cs/*.cs` (System.Private.CoreLib sources) and `tests/fixtures/presets/csharp-dotnet-runtime/` (the "Example File" of `docs/coding-guidelines/coding-style.md`) | <https://github.com/dotnet/runtime> | Copyright (c) .NET Foundation and Contributors. All rights reserved. |
| `bench/inputs/json/*.json` (from `test_parsing`) | <https://github.com/nst/JSONTestSuite> | Copyright (c) 2016 Nicolas Seriot |
| `bench/inputs/yaml/*.yaml` (from the `data` branch) | <https://github.com/yaml/yaml-test-suite> | Copyright (c) 2016-2020 Ingy döt Net |
| `tests/fixtures/presets/json-prettier-compat/`, `yaml-prettier-compat/`, and `playbook` in `yaml-default/` (Prettier test inputs and snapshots) | <https://github.com/prettier/prettier> | Copyright © James Long and contributors |
| `tests/fixtures/presets/csharp-microsoft/` (code blocks of "Common C# code conventions") | <https://github.com/dotnet/docs> (`LICENSE-CODE`) | Copyright (c) Microsoft Corporation |

The conformance corpora under `tests/conformance/corpus/` are fetched by the scripts next to them
and are not committed.

## Other sources

* `tests/fixtures/presets/csharp-google/`: the example of the "C# at Google Style Guide"
  (<https://google.github.io/styleguide/csharpguide.html>), licensed CC BY 3.0, modified as
  described in that directory's `SOURCE.md`.
* `tests/fixtures/presets/yaml-default/`: Examples 2.3, 2.9 and 2.10 of the YAML 1.2.2
  specification (<https://yaml.org/spec/1.2.2/>), which is CC BY 3.0.
* `tests/fixtures/presets/json-default/`: the examples of RFC 8259 section 13, used under the
  IETF Trust's Legal Provisions for code components (Revised BSD License).

## The MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
