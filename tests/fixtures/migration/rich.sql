-- Synthetic data expressed against the original Gophish 0.12.1 schema.
-- This same input is used on SQLite and MySQL; no Ruphish schema appears here.
DELETE FROM group_targets;
DELETE FROM targets;
DELETE FROM `groups`;
DELETE FROM users;
INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked,last_login) VALUES
(17,'migration-admin','fixture-hash','admin-key',1,0,0,'2020-02-29 12:34:56+00:00'),
(18,'migration-user','fixture-hash','user-key',2,0,0,'2020-02-29 12:34:56+00:00'),
(19,'migration-locked','fixture-hash','locked-key',2,0,1,NULL),
(20,'migration-reset','fixture-hash','reset-key',2,1,0,NULL);
INSERT INTO `groups`(id,user_id,name,modified_date) VALUES
(21,17,'Équipe 東京','2020-02-29 12:34:56+00:00'),(22,17,'Overlapping','2020-02-29 12:34:56+00:00'),
(23,18,'Équipe 東京','2020-02-29 12:34:56+00:00'),(24,17,'Empty','2020-02-29 12:34:56+00:00');
INSERT INTO targets(id,email,first_name,last_name,position) VALUES
(31,'Shared@Example.test','Zoë','李','Current role'),(32,'second@example.test','','',''),
(33,'other@example.test','Other','Owner','Analyst');
INSERT INTO group_targets(group_id,target_id) VALUES(21,31),(21,32),(22,31),(23,33);
INSERT INTO templates(id,user_id,name,subject,text,html,envelope_sender,modified_date) VALUES
(41,17,'Mixed content','Hello {{.FirstName}}','Plain {{.URL}}','<p>Grüße {{.FirstName}} — 東京</p>','Visible <visible@example.test>','2020-02-29 12:34:56+00:00'),
(42,18,'Mixed content','','Plain only','','','2020-02-29 12:34:56+00:00');
INSERT INTO attachments(id,template_id,name,type,content) VALUES
(43,41,'資料.bin','application/octet-stream','AAEC/w=='),(44,41,'empty.txt','text/plain','');
INSERT INTO pages(id,user_id,name,html,capture_credentials,capture_passwords,redirect_url,modified_date) VALUES
(47,17,'Landing','<form><input name="password" type="password"></form>',1,1,'https://learn.example.test/finish?q=1','2020-02-29 12:34:56+00:00'),
(48,18,'Landing','<p>Other owner</p>',0,0,'','2020-02-29 12:34:56+00:00');
INSERT INTO smtp(id,user_id,name,interface_type,host,username,password,from_address,ignore_cert_errors,modified_date) VALUES
(67,17,'Sender','SMTP','smtp.example.test:587','service','smtp-secret','Sender <sender@example.test>',1,'2020-02-29 12:34:56+00:00'),
(68,18,'Sender','SMTP','other.example.test:25','','','other@example.test',0,'2020-02-29 12:34:56+00:00');
INSERT INTO headers(id,smtp_id,`key`,value) VALUES(71,67,'X-Training','{{.RId}}'),(72,67,'Reply-To','reply@example.test');
INSERT INTO campaigns(id,user_id,name,status,template_id,page_id,smtp_id,url,created_date,launch_date,send_by_date,completed_date) VALUES
(53,17,'Active','In progress',41,47,67,'https://training.example.test:8443/path?source=legacy','2020-02-29 12:34:56+00:00','2020-03-01 00:00:00+00:00','2020-03-01 01:00:00+00:00',NULL),
(54,17,'Finished with deleted resources','Completed',49,50,69,'http://192.0.2.20:8080/training','2020-02-29 12:34:56+00:00','2020-03-01 00:00:00+00:00',NULL,'2020-03-02 00:00:00+00:00'),
(55,18,'Queued','Queued',42,48,68,'https://other.example.test/','2020-02-29 12:34:56+00:00','2030-01-01 00:00:00+00:00',NULL,NULL),
(56,17,'Missing page','In progress',41,50,67,'https://training.example.test:8443/path','2020-02-29 12:34:56+00:00','2020-03-01 00:00:00+00:00',NULL,NULL);
INSERT INTO results(id,campaign_id,user_id,r_id,email,first_name,last_name,position,status,reported,ip,latitude,longitude,send_date,modified_date) VALUES
(101,53,17,'submitted','Shared@Example.test','Zoë','李','Historical role','Submitted Data',1,'192.0.2.99',51.5,7.2,'2020-03-01 00:00:00+00:00','2020-03-01 00:05:00+00:00'),
(102,53,17,'opened','second@example.test','','','','Email Opened',0,'',0,0,'2020-03-01 00:00:00+00:00','2020-03-01 00:02:00+00:00'),
(103,53,17,'clicked','clicked@example.test','','','','Clicked Link',0,'',0,0,'2020-03-01 00:00:00+00:00','2020-03-01 00:03:00+00:00'),
(104,53,17,'sent','sent@example.test','','','','Email Sent',0,'',0,0,'2020-03-01 00:00:00+00:00','2020-03-01 00:00:00+00:00'),
(105,53,17,'error','error@example.test','','','','Error',0,'',0,0,NULL,'2020-03-01 00:00:00+00:00'),
(106,53,17,'pending','pending@example.test','','','','Sending',0,'',0,0,NULL,NULL),
(107,53,17,'inflight','inflight@example.test','','','','Retrying',0,'',0,0,NULL,NULL),
(108,54,17,'completed','completed@example.test','','','','Email Sent',0,'',0,0,'2020-03-01 00:00:00+00:00','2020-03-01 00:00:00+00:00'),
(109,55,18,'scheduled','other@example.test','Original','Owner','Analyst','Scheduled',0,'',0,0,NULL,NULL),
(110,56,17,'blocked','blocked@example.test','','','','Scheduled',0,'',0,0,NULL,NULL);
INSERT INTO events(id,campaign_id,email,time,message,details) VALUES
(201,53,'','2020-02-29 12:34:56+00:00','Campaign Created',''),
(202,53,'Shared@Example.test','2020-03-01 00:00:00+00:00','Email Sent',''),
(203,53,'Shared@Example.test','2020-03-01 00:01:00+00:00','Email Opened','{"browser":{"address":"PRIVATE-ADDRESS"}}'),
(204,53,'Shared@Example.test','2020-03-01 00:02:00+00:00','Email Opened',''),
(205,53,'Shared@Example.test','2020-03-01 00:03:00+00:00','Clicked Link',''),
(206,53,'Shared@Example.test','2020-03-01 00:04:00+00:00','Submitted Data','{"payload":{"password":["PRIVATE-PASSWORD"]}}'),
(207,53,'Shared@Example.test','2020-03-01 00:05:00+00:00','Email Reported',''),
(208,53,'error@example.test','2020-03-01 00:00:00+00:00','Error Sending Email','{"error":"PRIVATE-TRANSPORT"}'),
(209,53,'missing@example.test','2020-03-01 00:06:00+00:00','Email Opened','');
INSERT INTO mail_logs(id,campaign_id,user_id,r_id,send_date,send_attempt,processing) VALUES
(301,53,17,'pending','2030-01-01 00:00:00+00:00',0,0),
(302,53,17,'inflight','2030-01-01 00:00:00+00:00',8,1),
(303,54,17,'completed','2030-01-01 00:00:00+00:00',0,0),
(304,55,18,'scheduled','2030-01-01 00:00:00+00:00',8,0),
(305,56,17,'blocked','2030-01-01 00:00:00+00:00',0,0);
INSERT INTO imap(user_id,host,port,username,password,tls,enabled,folder,restrict_domain,delete_reported_campaign_email,imap_freq,modified_date,last_login,ignore_cert_errors) VALUES
(17,'imap.example.test',993,'reports','imap-secret',1,1,'Reports','example.test',1,120,'2020-02-29 12:34:56+00:00','2020-03-01 00:00:00+00:00',1),
(18,'',0,'','',0,1,'','',0,1,NULL,NULL,0);
INSERT INTO webhooks(id,name,url,secret,is_active) VALUES
(81,'Active hook','https://receiver.example.test/hook','webhook-secret',1),
(82,'Disabled hook','https://receiver.example.test/off','',0);
INSERT INTO email_requests(id,user_id,template_id,page_id,first_name,last_name,email,position,url,r_id,from_address) VALUES
(91,17,41,47,'Test','Recipient','test@example.test','Preview','https://training.example.test:8443/path','test-rid','sender@example.test');
