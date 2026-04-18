-- rate_limit.lua
-- Sliding window rate limiter: ZREMRANGEBYSCORE + ZCARD + ZADD + PEXPIRE
--
-- KEYS[1] = tenant:{tenant_id}:ratelimit:{resource}:{identity}  (sorted set)
--
-- ARGV[1] = max_requests (number)
-- ARGV[2] = now_ms (timestamp milliseconds)
-- ARGV[3] = window_ms (window duration milliseconds)
-- ARGV[4] = unique_request_id (UUID string)
--
-- Returns JSON: {status, remaining}
--   status: OK | RATE_LIMITED

local key = KEYS[1]

local max_requests = tonumber(ARGV[1])
local now_ms       = tonumber(ARGV[2])
local window_ms    = tonumber(ARGV[3])
local request_id   = ARGV[4]

-- Step 1: REMOVE expired entries outside the sliding window
local window_start = now_ms - window_ms
redis.call('ZREMRANGEBYSCORE', key, 0, window_start)

-- Step 2: COUNT current requests in window
local current_count = redis.call('ZCARD', key)

-- Step 3: CHECK if rate limit exceeded
if current_count >= max_requests then
    -- Calculate remaining TTL until oldest entry expires
    local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
    local remaining_ms = 0
    if #oldest >= 2 then
        remaining_ms = tonumber(oldest[2]) + window_ms - now_ms
        if remaining_ms < 0 then remaining_ms = 0 end
    end
    return '{"status":"RATE_LIMITED","remaining":0,"retry_after_ms":' .. remaining_ms .. '}'
end

-- Step 4: ADD request (score = timestamp, member = unique request ID)
redis.call('ZADD', key, now_ms, request_id)

-- Step 5: SET TTL on the sorted set (auto-cleanup)
redis.call('PEXPIRE', key, window_ms)

-- Step 6: Return OK + remaining requests
local remaining = max_requests - current_count - 1
return '{"status":"OK","remaining":' .. remaining .. ',"retry_after_ms":0}'
