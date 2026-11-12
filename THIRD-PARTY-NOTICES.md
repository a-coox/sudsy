# Third-party notices

Sudsy's own code is MIT ([LICENSE](LICENSE)). These files in the repository come from other
projects. Apart from the logo and the CSS name tables, none of them is part of the released binaries; they are test and benchmark inputs.

* `assets/logo/`: the Soap emoji of [Fluent UI Emoji](https://github.com/microsoft/fluentui-emoji) (commit `1ffb34c`), Copyright (c) Microsoft Corporation, MIT ([assets/logo/LICENSE](assets/logo/LICENSE)). Used as the logo of the extension, site and README.
* `crates/lang-css/data/`: the names of CSS (properties, at-rules, units, functions, selectors, ...) embedded in the binary: `names.txt` is generated from [known-css-properties](https://github.com/known-css/known-css-properties) 0.37.0 (MIT, Copyright (c) 2017 Mavrix Technologies) and [mdn-data](https://github.com/mdn/data) 2.37.2 (CC0-1.0); `extras.txt` adds names from [stylelint](https://github.com/stylelint/stylelint) 17.16.0 and its dependencies css-functions-list, html-tags, svg-tags and mathml-tag-names, and from [autoprefixer](https://github.com/postcss/autoprefixer) 10.2.5 (all MIT, Copyright (c) their authors).
* `crates/lang-html/data/elements.txt`: the element metadata of HTML (names, void and deprecated elements, attribute flags and values) embedded in the binary, generated from [html-validate](https://github.com/html-validate/html-validate) 11.21.0 (MIT, Copyright (c) 2017 David Sveningsson) by `data/generate.mjs`.

## MIT-licensed files

Each project below is licensed under the MIT License, whose text is given once at the end, with
the copyright line of that project.

| Files in this repository | Source | Copyright |
| --- | --- | --- |
| `bench/inputs/cs/*.cs` (System.Private.CoreLib sources) and `tests/fixtures/presets/csharp-dotnet-runtime/` (the "Example File" of `docs/coding-guidelines/coding-style.md`) | <https://github.com/dotnet/runtime> | Copyright (c) .NET Foundation and Contributors. All rights reserved. |
| `bench/inputs/json/*.json` (from `test_parsing`) | <https://github.com/nst/JSONTestSuite> | Copyright (c) 2016 Nicolas Seriot |
| `bench/inputs/yaml/*.yaml` (from the `data` branch) | <https://github.com/yaml/yaml-test-suite> | Copyright (c) 2016-2020 Ingy döt Net |
| `bench/inputs/js/mui-*` (`Button.js`, `Autocomplete.js` of v6.1.0) | <https://github.com/mui/material-ui> | Copyright (c) 2014 Call-Em-All |
| `bench/inputs/js/vue-renderer.ts` (`runtime-core/src/renderer.ts` of v3.5.13) | <https://github.com/vuejs/core> | Copyright (c) 2018-present, Yuxi (Evan) You and Vue contributors |
| `bench/inputs/js/excalidraw-color-picker.tsx` (v0.18.0) | <https://github.com/excalidraw/excalidraw> | Copyright (c) 2020 Excalidraw |
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

* `bench/inputs/tf/*.tf`: files of <https://github.com/terraform-aws-modules/terraform-aws-eks> and
  <https://github.com/terraform-aws-modules/terraform-aws-vpc>, Copyright (c) Anton Babenko and
  contributors, Apache License 2.0 (<https://www.apache.org/licenses/LICENSE-2.0>), unmodified.

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
