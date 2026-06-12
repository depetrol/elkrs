#!/bin/bash
# Regenerates goldens/expected/*.json by running the Java oracle on every
# case in goldens/cases/.
set -e
cd "$(dirname "$0")/.."
mkdir -p goldens/expected
for case in goldens/cases/*.json; do
  name=$(basename "$case" .json)
  java -jar oracle/target/elk-oracle-1.0.jar "$case" > "goldens/expected/$name.json"
  echo "generated goldens/expected/$name.json"
done
