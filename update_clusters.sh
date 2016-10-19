#!/bin/bash

# Exit on error
set -e
# Do not return wildcards if glob returns no matches
shopt -s nullglob

(bitcoin-cli getinfo && (bitcoin-cli stop || killall -9 bitcoind)) || true
sleep 5

if [ ! -f ~/clusterizer/chain.json ]; then
  echo "chain.json not found, running from scratch!"
  MODE="--new"
else
  echo "Resuming txoutdump..."
  MODE="--resume"
  cp -f ~/clusterizer/chain.json ~/clusterizer/chain.json.old
  cp -f ~/clusterizer/chain.json ~/clusterizer/chain.json.old-$(date -Iseconds)
fi

rusty-blockparser -t 16 -v "${MODE}" --chain-storage ~/clusterizer/chain.json txoutdump ~/clusterizer

for csvfile in ~/clusterizer/tx_out-*.csv
do
  echo "Sorting ${csvfile}..."
  LC_ALL=C sort -c "${csvfile}" || LC_ALL=C sort -u --parallel=16 "${csvfile}" -o "${csvfile}"
  echo "Done."
done

echo "Running clusterizer..."
rusty-blockparser -t 16 -v "${MODE}" --chain-storage ~/clusterizer/chain.json.old clusterizer ~/clusterizer

echo "Sorting clusters.csv..."
LC_ALL=C sort --parallel=16 ~/clusterizer/clusters.csv -o ~/clusterizer/clusters.csv
echo "Done."
bitcoind
