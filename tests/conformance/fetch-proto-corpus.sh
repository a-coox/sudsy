#!/usr/bin/env bash
# Fetches the pinned Protobuf corpus into tests/conformance/corpus/proto/<repo> (gitignored): the
# googleapis common and service protos, protobuf's own protos (proto2, proto3 and editions test
# files included) and grpc's.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs proto <<'EOF'
googleapis/googleapis 855217a6b3054992d6d0cb58370312bda024af04 */google/api/*.proto */google/rpc/*.proto */google/type/*.proto */google/longrunning/*.proto */google/pubsub/v1/*.proto */google/storage/v2/*.proto */google/cloud/vision/v1/*.proto */google/cloud/speech/v2/*.proto
protocolbuffers/protobuf 6f7be00e12b54b828a462bbf794e6f9be26b5e52 */src/google/protobuf/*.proto */conformance/*.proto */editions/golden/*.proto
grpc/grpc df982a7fb027d5276ec4a53c12ec0a1e0341fcf2 */src/proto/*.proto
EOF
