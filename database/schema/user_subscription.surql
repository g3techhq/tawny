-- `channel` and `video` stay shared. They are YouTube's facts, not the
-- viewer's: two accounts on one server should not fetch and store the same
-- upload twice. Everything that is genuinely a *choice* - what you follow, what
-- you saved, where you stopped watching - hangs off the account instead.
--
-- This table is who follows what. Presence of a row is not enough on its own:
-- an explicit `subscribed` flag lets an unsubscribe be recorded and synced
-- rather than inferred from a missing row, which is what the client already
-- sends.
--
-- `channel.subscribed` survives beside this as a derived flag meaning "someone
-- on this instance follows it". That is precisely the question the poller and
-- the WebSub renewals ask, so they keep working untouched - and it must be
-- recomputed from this table whenever a subscription changes, never written
-- directly. See `refresh_channel_interest`.
DEFINE TABLE IF NOT EXISTS user_subscription TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD IF NOT EXISTS owner ON user_subscription TYPE record<app_user>;
DEFINE FIELD IF NOT EXISTS channel_id ON user_subscription TYPE string;
DEFINE FIELD IF NOT EXISTS subscribed ON user_subscription TYPE bool DEFAULT true;
DEFINE FIELD IF NOT EXISTS content ON user_subscription TYPE string DEFAULT "all";
DEFINE FIELD IF NOT EXISTS updated_at ON user_subscription TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS user_subscription_owner_channel_idx ON user_subscription
  FIELDS owner, channel_id UNIQUE;
DEFINE INDEX IF NOT EXISTS user_subscription_channel_idx ON user_subscription FIELDS channel_id;
