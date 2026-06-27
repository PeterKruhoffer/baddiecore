-- Non-destructive local development content. Dependencies are resolved through
-- stable natural keys, so this also works when matching rows use different IDs.

START TRANSACTION;

INSERT INTO cms_templates (id, template_key, name, description) VALUES
  (UUID(), 'folder', 'Folder', 'Organizes non-page content.'),
  (UUID(), 'home-page', 'Home Page', 'Landing page with composed renderings.'),
  (UUID(), 'standard-page', 'Standard Page', 'General content page.'),
  (UUID(), 'hero-content', 'Hero Content', 'Content used by a hero rendering.'),
  (UUID(), 'rich-text-content', 'Rich Text Content', 'Content used by a rich text rendering.'),
  (UUID(), 'cta-content', 'CTA Content', 'Content used by a call-to-action rendering.')
ON DUPLICATE KEY UPDATE template_key = cms_templates.template_key;

SET @folder_template_id := (SELECT id FROM cms_templates WHERE template_key = 'folder');
SET @home_page_template_id := (SELECT id FROM cms_templates WHERE template_key = 'home-page');
SET @standard_page_template_id := (SELECT id FROM cms_templates WHERE template_key = 'standard-page');
SET @hero_template_id := (SELECT id FROM cms_templates WHERE template_key = 'hero-content');
SET @rich_text_template_id := (SELECT id FROM cms_templates WHERE template_key = 'rich-text-content');
SET @cta_template_id := (SELECT id FROM cms_templates WHERE template_key = 'cta-content');

INSERT INTO cms_template_fields
  (id, template_id, field_key, name, field_type, is_required, sort_order) VALUES
  (UUID(), @home_page_template_id, 'title', 'Title', 'text', 1, 10),
  (UUID(), @home_page_template_id, 'description', 'Description', 'textarea', 0, 20),
  (UUID(), @standard_page_template_id, 'title', 'Title', 'text', 1, 10),
  (UUID(), @standard_page_template_id, 'description', 'Description', 'textarea', 0, 20),
  (UUID(), @hero_template_id, 'eyebrow', 'Eyebrow', 'text', 0, 10),
  (UUID(), @hero_template_id, 'heading', 'Heading', 'text', 1, 20),
  (UUID(), @hero_template_id, 'body', 'Body', 'textarea', 0, 30),
  (UUID(), @rich_text_template_id, 'heading', 'Heading', 'text', 0, 10),
  (UUID(), @rich_text_template_id, 'body', 'Body', 'textarea', 1, 20),
  (UUID(), @cta_template_id, 'heading', 'Heading', 'text', 1, 10),
  (UUID(), @cta_template_id, 'buttonText', 'Button text', 'text', 1, 20),
  (UUID(), @cta_template_id, 'buttonUrl', 'Button URL', 'text', 1, 30)
ON DUPLICATE KEY UPDATE field_key = cms_template_fields.field_key;

INSERT INTO cms_workflow_states
  (id, state_key, name, allows_editing, allows_publishing, sort_order) VALUES
  (UUID(), 'draft', 'Draft', 1, 0, 10),
  (UUID(), 'awaiting-approval', 'Awaiting Approval', 0, 0, 20),
  (UUID(), 'approved', 'Approved', 0, 1, 30),
  (UUID(), 'published', 'Published', 0, 0, 40)
ON DUPLICATE KEY UPDATE state_key = cms_workflow_states.state_key;

SET @draft_state_id := (SELECT id FROM cms_workflow_states WHERE state_key = 'draft');
SET @awaiting_approval_state_id := (SELECT id FROM cms_workflow_states WHERE state_key = 'awaiting-approval');
SET @approved_state_id := (SELECT id FROM cms_workflow_states WHERE state_key = 'approved');
SET @published_state_id := (SELECT id FROM cms_workflow_states WHERE state_key = 'published');

INSERT INTO cms_workflow_actions
  (id, action_key, name, from_state_id, to_state_id, sort_order) VALUES
  (UUID(), 'submit', 'Submit for approval', @draft_state_id, @awaiting_approval_state_id, 10),
  (UUID(), 'approve', 'Approve', @awaiting_approval_state_id, @approved_state_id, 20),
  (UUID(), 'reject', 'Reject', @awaiting_approval_state_id, @draft_state_id, 30),
  (UUID(), 'publish', 'Publish', @approved_state_id, @published_state_id, 40)
ON DUPLICATE KEY UPDATE action_key = cms_workflow_actions.action_key;

SET @submit_action_id := (SELECT id FROM cms_workflow_actions WHERE action_key = 'submit');
SET @approve_action_id := (SELECT id FROM cms_workflow_actions WHERE action_key = 'approve');
SET @reject_action_id := (SELECT id FROM cms_workflow_actions WHERE action_key = 'reject');
SET @publish_action_id := (SELECT id FROM cms_workflow_actions WHERE action_key = 'publish');

INSERT INTO cms_workflow_action_roles
  (id, workflow_action_id, role_key) VALUES
  (UUID(), @submit_action_id, 'author'),
  (UUID(), @submit_action_id, 'content-admin'),
  (UUID(), @approve_action_id, 'content-admin'),
  (UUID(), @reject_action_id, 'content-admin'),
  (UUID(), @publish_action_id, 'content-admin')
ON DUPLICATE KEY UPDATE role_key = cms_workflow_action_roles.role_key;

INSERT INTO cms_rendering_definitions
  (id, rendering_key, name, component_key, datasource_template_id) VALUES
  (UUID(), 'hero', 'Hero', 'hero', @hero_template_id),
  (UUID(), 'rich-text', 'Rich Text', 'rich-text', @rich_text_template_id),
  (UUID(), 'cta', 'Call to Action', 'cta', @cta_template_id)
ON DUPLICATE KEY UPDATE rendering_key = cms_rendering_definitions.rendering_key;

SET @hero_rendering_id := (SELECT id FROM cms_rendering_definitions WHERE rendering_key = 'hero');
SET @rich_text_rendering_id := (SELECT id FROM cms_rendering_definitions WHERE rendering_key = 'rich-text');
SET @cta_rendering_id := (SELECT id FROM cms_rendering_definitions WHERE rendering_key = 'cta');

INSERT INTO cms_rendering_fields
  (id, rendering_definition_id, field_key, name, field_type, is_required, sort_order) VALUES
  (UUID(), @hero_rendering_id, 'eyebrow', 'Eyebrow', 'text', 0, 10),
  (UUID(), @hero_rendering_id, 'heading', 'Heading', 'text', 1, 20),
  (UUID(), @hero_rendering_id, 'body', 'Body', 'textarea', 0, 30),
  (UUID(), @rich_text_rendering_id, 'heading', 'Heading', 'text', 0, 10),
  (UUID(), @rich_text_rendering_id, 'body', 'Body', 'textarea', 1, 20),
  (UUID(), @cta_rendering_id, 'heading', 'Heading', 'text', 1, 10),
  (UUID(), @cta_rendering_id, 'buttonText', 'Button text', 'text', 1, 20),
  (UUID(), @cta_rendering_id, 'buttonUrl', 'Button URL', 'text', 1, 30)
ON DUPLICATE KEY UPDATE field_key = cms_rendering_fields.field_key;

INSERT INTO cms_rendering_allowed_placeholders
  (id, rendering_definition_id, placeholder_name) VALUES
  (UUID(), @hero_rendering_id, 'main'),
  (UUID(), @rich_text_rendering_id, 'main'),
  (UUID(), @cta_rendering_id, 'main')
ON DUPLICATE KEY UPDATE placeholder_name = cms_rendering_allowed_placeholders.placeholder_name;

INSERT INTO cms_content_items
  (id, parent_id, item_key, name, path, template_id, workflow_state_id, sort_order, version, is_draft) VALUES
  (UUID(), NULL, 'home', 'Home', '/', @home_page_template_id, @draft_state_id, 10, 1, 1),
  (UUID(), NULL, 'component-data', 'Component Data', '/_data', @folder_template_id, @draft_state_id, 20, 1, 1)
ON DUPLICATE KEY UPDATE path = cms_content_items.path;

SET @home_item_id := (SELECT id FROM cms_content_items WHERE path = '/');
SET @data_item_id := (SELECT id FROM cms_content_items WHERE path = '/_data');

INSERT INTO cms_content_items
  (id, parent_id, item_key, name, path, template_id, workflow_state_id, sort_order, version, is_draft) VALUES
  (UUID(), @home_item_id, 'about', 'About', '/about', @standard_page_template_id, @published_state_id, 10, 1, 0),
  (UUID(), @data_item_id, 'home-hero', 'Home Hero', '/_data/home-hero', @hero_template_id, @draft_state_id, 10, 1, 1),
  (UUID(), @data_item_id, 'home-introduction', 'Home Introduction', '/_data/home-introduction', @rich_text_template_id, @draft_state_id, 20, 1, 1),
  (UUID(), @data_item_id, 'home-cta', 'Home CTA', '/_data/home-cta', @cta_template_id, @draft_state_id, 30, 1, 1),
  (UUID(), @data_item_id, 'about-body', 'About Body', '/_data/about-body', @rich_text_template_id, @published_state_id, 40, 1, 0)
ON DUPLICATE KEY UPDATE path = cms_content_items.path;

SET @about_item_id := (SELECT id FROM cms_content_items WHERE path = '/about');
SET @home_hero_item_id := (SELECT id FROM cms_content_items WHERE path = '/_data/home-hero');
SET @home_intro_item_id := (SELECT id FROM cms_content_items WHERE path = '/_data/home-introduction');
SET @home_cta_item_id := (SELECT id FROM cms_content_items WHERE path = '/_data/home-cta');
SET @about_body_item_id := (SELECT id FROM cms_content_items WHERE path = '/_data/about-body');

INSERT INTO cms_content_field_values
  (id, content_item_id, field_key, field_value) VALUES
  (UUID(), @home_item_id, 'title', 'Baddiecore'),
  (UUID(), @home_item_id, 'description', 'A Linux-first, headless CMS built on modern .NET.'),
  (UUID(), @about_item_id, 'title', 'About Baddiecore'),
  (UUID(), @about_item_id, 'description', 'Why this CMS exists and what it is designed to prove.'),
  (UUID(), @home_hero_item_id, 'eyebrow', 'Linux-first CMS'),
  (UUID(), @home_hero_item_id, 'heading', 'Content infrastructure without the Windows-shaped anchor'),
  (UUID(), @home_hero_item_id, 'body', 'Build, preview, approve, and publish headless content from a modern .NET platform.'),
  (UUID(), @home_intro_item_id, 'heading', 'A real end-to-end content path'),
  (UUID(), @home_intro_item_id, 'body', 'This page is loaded from MySQL through the Baddiecore layout API and rendered by the SolidJS dashboard.'),
  (UUID(), @home_cta_item_id, 'heading', 'The foundation is connected.'),
  (UUID(), @home_cta_item_id, 'buttonText', 'Read about the project'),
  (UUID(), @home_cta_item_id, 'buttonUrl', '/about'),
  (UUID(), @about_body_item_id, 'heading', 'A small, deliberate CMS'),
  (UUID(), @about_body_item_id, 'body', 'Baddiecore starts with the core authoring and delivery loop, then grows from working vertical slices.')
ON DUPLICATE KEY UPDATE field_key = cms_content_field_values.field_key;

INSERT INTO cms_page_renderings
  (id, content_item_id, rendering_definition_id, datasource_item_id, placeholder_name, sort_order)
SELECT UUID(), @home_item_id, @hero_rendering_id, @home_hero_item_id, 'main', 10
WHERE NOT EXISTS (
  SELECT 1 FROM cms_page_renderings
  WHERE content_item_id = @home_item_id
    AND rendering_definition_id = @hero_rendering_id
    AND datasource_item_id = @home_hero_item_id
    AND placeholder_name = 'main'
);

INSERT INTO cms_page_renderings
  (id, content_item_id, rendering_definition_id, datasource_item_id, placeholder_name, sort_order)
SELECT UUID(), @home_item_id, @rich_text_rendering_id, @home_intro_item_id, 'main', 20
WHERE NOT EXISTS (
  SELECT 1 FROM cms_page_renderings
  WHERE content_item_id = @home_item_id
    AND rendering_definition_id = @rich_text_rendering_id
    AND datasource_item_id = @home_intro_item_id
    AND placeholder_name = 'main'
);

INSERT INTO cms_page_renderings
  (id, content_item_id, rendering_definition_id, datasource_item_id, placeholder_name, sort_order)
SELECT UUID(), @home_item_id, @cta_rendering_id, @home_cta_item_id, 'main', 30
WHERE NOT EXISTS (
  SELECT 1 FROM cms_page_renderings
  WHERE content_item_id = @home_item_id
    AND rendering_definition_id = @cta_rendering_id
    AND datasource_item_id = @home_cta_item_id
    AND placeholder_name = 'main'
);

INSERT INTO cms_page_renderings
  (id, content_item_id, rendering_definition_id, datasource_item_id, placeholder_name, sort_order)
SELECT UUID(), @about_item_id, @rich_text_rendering_id, @about_body_item_id, 'main', 10
WHERE NOT EXISTS (
  SELECT 1 FROM cms_page_renderings
  WHERE content_item_id = @about_item_id
    AND rendering_definition_id = @rich_text_rendering_id
    AND datasource_item_id = @about_body_item_id
    AND placeholder_name = 'main'
);

COMMIT;
