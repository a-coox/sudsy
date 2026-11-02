# yaml/default@1 samples

* `mapping-scalars-to-sequences`, `ranking-comments`, `anchors`: YAML 1.2.2 specification,
  Examples 2.3, 2.9 (first document) and 2.10 (<https://yaml.org/spec/1.2.2/#chapter-2-language-overview>).
  The `.out` files are the specification's text; the `.in` files use four-space indentation.
* `playbook`: Prettier's `tests/format/yaml/ansible/playbook.yml`
  (<https://github.com/prettier/prettier/blob/main/tests/format/yaml/ansible/playbook.yml>). It is
  its own expected output: `yaml/default` keeps the sequence indentation it finds. The same input
  goes through `yaml/prettier-compat@1` in `yaml-prettier-compat`.
