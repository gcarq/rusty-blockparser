#!/bin/bash

# Show commands, expanding variables
set -x
# Exit on error
set -e
# Do not return wildcards if glob returns no matches
shopt -s nullglob
# Show backtraces
export RUST_BACKTRACE=1

while pgrep -x "bitcoind" > /dev/null
do
  echo "Stopping bitcoind..."
  bitcoin-cli stop
  sleep 10
done

BLOCKPARSER="/usr/local/bin/rusty-blockparser"
NPROC=`nproc`
OLDCHAINS=(~/clusterizer/chain.json.old-*)

if [ -e "${OLDCHAINS[0]}" ]; then
  # Determine the last sane chain.json from frozen versions
  for (( i=${#OLDCHAINS[@]}-1 ; i>=0 ; i-- )) ; do
    CHAINFILE="${OLDCHAINS[i]}"
    HASHESLEN=`tail -c 100 "${CHAINFILE}" | cut -d: -f2- | cut -d, -f1`
    INDEX=`tail -c 100 "${CHAINFILE}" | cut -d: -f3- | cut -d, -f1`
    if [ $HASHESLEN -eq $INDEX ]; then
      echo "Last sane chain.json: ${CHAINFILE}, up to block ${INDEX}."
      break
    fi
  done
fi

if [ -e "${OLDCHAINS[0]}" ]; then
  echo "Resuming..."
  RESUME_OR_REINDEX="--resume"
  cp -f "${CHAINFILE}" ~/clusterizer/chain.json
else
  echo "Running from scratch!"
  RESUME_OR_REINDEX="--reindex"
fi

echo "Running clusterizer..."
${BLOCKPARSER} -t ${NPROC} ${RESUME_OR_REINDEX} --chain-storage ~/clusterizer/chain.json clusterizer ~/clusterizer
echo "Done."

echo "Sorting clusters.csv..."
LC_ALL=C sort --parallel=${NPROC} ~/clusterizer/clusters.csv -o ~/clusterizer/clusters.csv
echo "Done."

# Copy chain.json to a frozen version
cp -f ~/clusterizer/chain.json ~/clusterizer/chain.json.old-$(date -Iseconds)

# Clean chain.json frozen versions older than one week
find ~/clusterizer -name 'chain.json.old-*' -mtime +7 -exec rm -f {} \;

# Clean temporary files
rm -f ~/clusterizer/chain.json
