#!/bin/bash

BLOCKPARSER="/usr/local/bin/rusty-blockparser"
NPROC=`nproc`

# Show commands, expanding variables
set -x
# Exit on error
set -e
# Do not return wildcards if glob returns no matches
shopt -s nullglob

while pgrep -x "bitcoind" > /dev/null
do
  echo "Stopping bitcoind..."
  bitcoin-cli stop
  sleep 10
done
echo "Done."

if [ ! -f ~/clusterizer/.skip-txoutdump ]; then
  OLDCHAINS=(~/clusterizer/chain.json.old-*)

  if [ -e "${OLDCHAINS[0]}" ]; then
    echo "Resuming txoutdump..."
    MODE="--resume"
    # Determine the last sane chain.json from frozen versions
    for (( i=${#OLDCHAINS[@]}-1 ; i>=0 ; i-- )) ; do
      CHAINFILE="${OLDCHAINS[i]}"
      HASHESLEN=`tail -c 100 "${CHAINFILE}" | cut -d: -f2- | cut -d, -f1`
      INDEX=`tail -c 100 "${CHAINFILE}" | cut -d: -f3- | cut -d, -f1`
      if [ $HASHESLEN -eq $INDEX ]; then
        echo "Last sane chain.json: ${CHAINFILE}, from block ${INDEX}."
        cp -f "${CHAINFILE}" ~/clusterizer/chain.json
        break
      fi
    done
    cp -f ~/clusterizer/chain.json ~/clusterizer/chain.json.old
  else
    echo "Running from scratch!"
    MODE="--reindex"
  fi

  ${BLOCKPARSER} -t ${NPROC} ${MODE} --chain-storage ~/clusterizer/chain.json txoutdump ~/clusterizer

  for csvfile in `find ~/clusterizer -name 'tx_out-*.csv' -mtime -1 -print` ; do
    echo "Sorting ${csvfile}..."
    LC_ALL=C sort -u --parallel=${NPROC} "${csvfile}" -o "${csvfile}"
    echo "Done."
  done

  # Copy chain.json to a frozen version
  cp -f ~/clusterizer/chain.json ~/clusterizer/chain.json.old-$(date -Iseconds)

  # Clean chain.json frozen versions older than one week
  find ~/clusterizer -name 'chain.json.old-*' -mtime +7 -exec rm -f {} \;
fi

# Create skip-file for txoutdump
touch ~/clusterizer/.skip-txoutdump

if [ ! -f ~/clusterizer/.skip-clusterizer ]; then
  MODE="--resume"
  cp -f ~/clusterizer/chain.json.old /tmp/chain.json.old
  echo "Running clusterizer..."
  ${BLOCKPARSER} -t ${NPROC} ${MODE} --chain-storage /tmp/chain.json.old clusterizer ~/clusterizer

  echo "Sorting clusters.csv..."
  LC_ALL=C sort --parallel=${NPROC} ~/clusterizer/clusters.csv -o ~/clusterizer/clusters.csv
  echo "Done."
fi

# Create skip-file for clusterizer
touch ~/clusterizer/.skip-clusterizer

# Clean temporary files
rm -f ~/clusterizer/chain.json ~/clusterizer/chain.json.old
