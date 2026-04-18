-- write_behind_dedup.lua
-- Atomic write-behind with stream deduplication
--
-- Guarantees:
--   1. Cache ALWAYS updated (latest state wins)
--   2. Pending SET ALWAYS has the entity ID (SADD is idempotent)
--   3. Stream gets ONE entry per state change (within dedup window)
--   4. No duplicates even on retry/multi-transition
--
-- KEYS[1] = entity data key       (ec:demande:data:{id})
-- KEYS[2] = pending set key       (ec:demande:wb:pending)
-- KEYS[3] = persist stream key    (ec:persist:demande)
-- KEYS[4] = dedup sentinel key    (ec:demande:dedup:{id}:{newStatut})
--
-- ARGV[1] = entity JSON (full DTO)
-- ARGV[2] = entity ID (string UUID)
-- ARGV[3] = old status (string, for audit trail)
-- ARGV[4] = new status (string, for audit trail + dedup key suffix)
-- ARGV[5] = TTL seconds for cache data (default 1814400 = 21 days)
-- ARGV[6] = dedup window seconds (default 5)
-- ARGV[7] = operator ID (string, for audit)
--
-- Returns:
--   "XADD:{stream_id}" if new stream entry was added
--   "DEDUP"             if deduped (cache updated, no stream entry)

-- Step 1: ALWAYS update the cache (latest state wins)
redis.call('SET', KEYS[1], ARGV[1], 'EX', tonumber(ARGV[5]))

-- Step 2: ALWAYS add to pending set (SADD = idempotent)
redis.call('SADD', KEYS[2], ARGV[2])

-- Step 3: Check dedup sentinel — was this exact state transition already recorded?
local sentinel_exists = redis.call('EXISTS', KEYS[4])
if sentinel_exists == 1 then
    -- Same entity+status was recently written to stream → skip XADD
    return 'DEDUP'
end

-- Step 4: XADD to persist stream (audit trail for this state change)
local stream_id = redis.call('XADD', KEYS[3], '*',
    'entityId', ARGV[2],
    'oldStatut', ARGV[3],
    'newStatut', ARGV[4],
    'operateurId', ARGV[7],
    'timestamp', tostring(redis.call('TIME')[1]))

-- Step 5: Set dedup sentinel with TTL (prevents duplicate XADD for same state)
redis.call('SET', KEYS[4], stream_id, 'EX', tonumber(ARGV[6]))

return 'XADD:' .. stream_id
