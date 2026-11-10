#!/usr/bin/env bash
# Fetches the pinned HCL corpus into tests/conformance/corpus/hcl/<repo> (gitignored): Terraform
# modules (aws vpc and eks), Terraform's own fmt and config test data, Nomad job files and Packer
# templates. Empty files are removed (`terraform fmt` turns one into a blank line), and so are the
# files in HCL1 syntax.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs hcl <<'EOF'
terraform-aws-modules/terraform-aws-vpc b3abd6df2ecf052451a361ed55b8f06f8742a795 *.tf
terraform-aws-modules/terraform-aws-eks 91b8a8d944e3cf169483108acd12d5b1e47856f7 *.tf
hashicorp/terraform 03b4f662e75682637b6ec862ee5cf6d1ab65ce1f */internal/command/testdata/fmt/*.tf */internal/configs/testdata/valid-files/*.tf */internal/configs/testdata/valid-modules/*/*.tf
hashicorp/nomad e77c361fd055c12659a194ed62eae477b6e45f40 */e2e/*/input/*.nomad */e2e/*/input/*.hcl */command/agent/testdata/*.hcl */command/agent/test-resources/*.hcl */demo/csi/*/*.hcl */terraform/shared/config/*.hcl
hashicorp/packer 7519edef169ed03146ef668f79bae30cfcb67716 */hcl2template/testdata/*/*.pkr.hcl */hcl2template/testdata/*.pkr.hcl */examples/*/*.pkr.hcl
EOF
corpus="$(dirname "$0")/corpus/hcl"
find "$corpus" -type f -empty -delete
# Nomad's older agent and volume files are HCL1 (`terraform fmt` rejects them too).
cd "$corpus/nomad"
rm -f command/agent/testdata/{basic,config-slices,obj-len-one}.hcl command/agent/testdata/sample1/sample2.hcl   terraform/shared/config/nomad_client.hcl e2e/devices/input/basic_device_config.hcl e2e/csi/input/ebs-volume{0,1}.hcl
