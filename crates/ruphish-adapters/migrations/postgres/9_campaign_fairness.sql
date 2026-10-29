-- Preserve the maximum over all deliveries of each candidate campaign.
CREATE INDEX ruphish_delivery_fairness ON ruphish_delivery(campaign_id,last_claim);
