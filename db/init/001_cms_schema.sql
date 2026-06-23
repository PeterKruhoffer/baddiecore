CREATE TABLE IF NOT EXISTS cms_templates (
  id CHAR(36) NOT NULL,
  template_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  description TEXT NULL,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_templates_template_key (template_key)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_template_fields (
  id CHAR(36) NOT NULL,
  template_id CHAR(36) NOT NULL,
  field_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  field_type VARCHAR(64) NOT NULL,
  is_required TINYINT(1) NOT NULL DEFAULT 0,
  sort_order INT NOT NULL DEFAULT 0,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_template_fields_template_field (template_id, field_key),
  CONSTRAINT fk_cms_template_fields_template
    FOREIGN KEY (template_id) REFERENCES cms_templates (id)
    ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_workflow_states (
  id CHAR(36) NOT NULL,
  state_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  allows_editing TINYINT(1) NOT NULL DEFAULT 0,
  allows_publishing TINYINT(1) NOT NULL DEFAULT 0,
  sort_order INT NOT NULL DEFAULT 0,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_workflow_states_state_key (state_key)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_workflow_actions (
  id CHAR(36) NOT NULL,
  action_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  from_state_id CHAR(36) NOT NULL,
  to_state_id CHAR(36) NOT NULL,
  sort_order INT NOT NULL DEFAULT 0,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_workflow_actions_action_key (action_key),
  CONSTRAINT fk_cms_workflow_actions_from_state
    FOREIGN KEY (from_state_id) REFERENCES cms_workflow_states (id),
  CONSTRAINT fk_cms_workflow_actions_to_state
    FOREIGN KEY (to_state_id) REFERENCES cms_workflow_states (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_workflow_action_roles (
  id CHAR(36) NOT NULL,
  workflow_action_id CHAR(36) NOT NULL,
  role_key VARCHAR(128) NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_workflow_action_roles_action_role (workflow_action_id, role_key),
  CONSTRAINT fk_cms_workflow_action_roles_action
    FOREIGN KEY (workflow_action_id) REFERENCES cms_workflow_actions (id)
    ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_content_items (
  id CHAR(36) NOT NULL,
  parent_id CHAR(36) NULL,
  item_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  path VARCHAR(512) NOT NULL,
  template_id CHAR(36) NOT NULL,
  workflow_state_id CHAR(36) NOT NULL,
  sort_order INT NOT NULL DEFAULT 0,
  version INT NOT NULL DEFAULT 1,
  is_draft TINYINT(1) NOT NULL DEFAULT 1,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_content_items_path (path),
  KEY ix_cms_content_items_parent_sort (parent_id, sort_order),
  CONSTRAINT fk_cms_content_items_parent
    FOREIGN KEY (parent_id) REFERENCES cms_content_items (id)
    ON DELETE SET NULL,
  CONSTRAINT fk_cms_content_items_template
    FOREIGN KEY (template_id) REFERENCES cms_templates (id),
  CONSTRAINT fk_cms_content_items_workflow_state
    FOREIGN KEY (workflow_state_id) REFERENCES cms_workflow_states (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_content_field_values (
  id CHAR(36) NOT NULL,
  content_item_id CHAR(36) NOT NULL,
  field_key VARCHAR(128) NOT NULL,
  field_value LONGTEXT NULL,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_content_field_values_item_field (content_item_id, field_key),
  CONSTRAINT fk_cms_content_field_values_item
    FOREIGN KEY (content_item_id) REFERENCES cms_content_items (id)
    ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_rendering_definitions (
  id CHAR(36) NOT NULL,
  rendering_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  component_key VARCHAR(200) NOT NULL,
  datasource_template_id CHAR(36) NULL,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_rendering_definitions_rendering_key (rendering_key),
  CONSTRAINT fk_cms_rendering_definitions_datasource_template
    FOREIGN KEY (datasource_template_id) REFERENCES cms_templates (id)
    ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_rendering_fields (
  id CHAR(36) NOT NULL,
  rendering_definition_id CHAR(36) NOT NULL,
  field_key VARCHAR(128) NOT NULL,
  name VARCHAR(200) NOT NULL,
  field_type VARCHAR(64) NOT NULL,
  is_required TINYINT(1) NOT NULL DEFAULT 0,
  sort_order INT NOT NULL DEFAULT 0,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_rendering_fields_rendering_field (rendering_definition_id, field_key),
  CONSTRAINT fk_cms_rendering_fields_rendering
    FOREIGN KEY (rendering_definition_id) REFERENCES cms_rendering_definitions (id)
    ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_rendering_allowed_placeholders (
  id CHAR(36) NOT NULL,
  rendering_definition_id CHAR(36) NOT NULL,
  placeholder_name VARCHAR(128) NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY ux_cms_rendering_allowed_placeholders_rendering_placeholder (rendering_definition_id, placeholder_name),
  CONSTRAINT fk_cms_rendering_allowed_placeholders_rendering
    FOREIGN KEY (rendering_definition_id) REFERENCES cms_rendering_definitions (id)
    ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS cms_page_renderings (
  id CHAR(36) NOT NULL,
  content_item_id CHAR(36) NOT NULL,
  rendering_definition_id CHAR(36) NOT NULL,
  datasource_item_id CHAR(36) NULL,
  placeholder_name VARCHAR(128) NOT NULL,
  sort_order INT NOT NULL DEFAULT 0,
  created_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at_utc DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  KEY ix_cms_page_renderings_item_placeholder_sort (content_item_id, placeholder_name, sort_order),
  CONSTRAINT fk_cms_page_renderings_content_item
    FOREIGN KEY (content_item_id) REFERENCES cms_content_items (id)
    ON DELETE CASCADE,
  CONSTRAINT fk_cms_page_renderings_rendering_definition
    FOREIGN KEY (rendering_definition_id) REFERENCES cms_rendering_definitions (id),
  CONSTRAINT fk_cms_page_renderings_datasource_item
    FOREIGN KEY (datasource_item_id) REFERENCES cms_content_items (id)
    ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;
