
CREATE OR REPLACE VIEW view_blocks AS
SELECT
	height,
	LOWER(HEX(hash)) AS hash,
	version,
	LOWER(HEX(hashPrev)) AS hashPrev,
	LOWER(HEX(hashMerkleRoot)) AS hashMerkleRoot,
	nTime,
	nBits,
	nNonce
FROM blocks
	ORDER BY height ASC;

SELECT * FROM view_blocks;


CREATE OR REPLACE VIEW view_transactions AS
SELECT
	LOWER(HEX(txid)) as txid,
    LOWER(HEX(hashBlock)) as hashBlock,
    version,
    lockTime
FROM transactions;

SELECT * FROM view_transactions;


CREATE OR REPLACE VIEW view_balances AS
    SELECT
        address,
        CAST(SUM(value) / 100000000 AS DECIMAL (16 , 8 )) AS balance
    FROM
        tx_out
    WHERE
        unspent = TRUE
    GROUP BY address
    ORDER BY balance DESC;


SELECT * FROM view_balances;


CREATE FUNCTION target (bits float)
	RETURNS REAL DETERMINISTIC
RETURN mod(bits, 0x1000000) * pow(256, bits div 0x1000000 - 3);
