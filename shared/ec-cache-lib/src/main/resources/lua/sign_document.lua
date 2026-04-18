-- sign_document.lua
-- Idempotent document signature: CHECK fingerprint + LOCK + XADD stream + SADD dedup + DEL lock
--
-- KEYS[1] = tenant:{tenant_id}:stream:signatures                (event stream)
-- KEYS[2] = tenant:{tenant_id}:signed:fingerprints              (SET, permanent dedup)
-- KEYS[3] = tenant:{tenant_id}:sign:lock:{fingerprint}          (temp lock key)
--
-- ARGV[1] = payload_json (string)
-- ARGV[2] = SHA-384 fingerprint (hex string)
-- ARGV[3] = lock_ttl_seconds (number, e.g., 30)
-- ARGV[4] = docType (string, e.g., "ACTE_NAISSANCE", "ORDONNANCE", "BULLETIN")
--
-- Returns JSON: {status, stream_id}
--   status: OK | ALREADY_SIGNED | CONCURRENT_SIGNATURE

local stream_key      = KEYS[1]
local fingerprints_key = KEYS[2]
local lock_key        = KEYS[3]

local payload_json = ARGV[1]
local fingerprint  = ARGV[2]
local lock_ttl     = tonumber(ARGV[3])
local doc_type     = ARGV[4]

-- Step 1: CHECK fingerprint (idempotency)
local already_signed = redis.call('SISMEMBER', fingerprints_key, fingerprint)
if already_signed == 1 then
    return '{"status":"ALREADY_SIGNED","stream_id":null}'
end

-- Step 2: LOCK (SET NX — prevents concurrent signature of same document)
local locked = redis.call('SET', lock_key, '1', 'EX', lock_ttl, 'NX')
if locked == false then
    return '{"status":"CONCURRENT_SIGNATURE","stream_id":null}'
end

-- Step 3: WRITE to stream
local stream_id = redis.call('XADD', stream_key, '*',
    'type', 'signature',
    'payload', payload_json,
    'fingerprint', fingerprint,
    'docType', doc_type)

-- Step 4: ADD fingerprint to permanent dedup set
redis.call('SADD', fingerprints_key, fingerprint)

-- Step 5: CLEANUP lock
redis.call('DEL', lock_key)

-- Step 6: Return OK + stream record ID
return '{"status":"OK","stream_id":"' .. stream_id .. '"}'
