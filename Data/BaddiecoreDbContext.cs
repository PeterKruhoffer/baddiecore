using Baddiecore.Data.Entities;
using Baddiecore.Data.Ids;
using Microsoft.EntityFrameworkCore;

namespace Baddiecore.Data;

public sealed class BaddiecoreDbContext(DbContextOptions<BaddiecoreDbContext> options)
    : DbContext(options)
{
    public DbSet<TemplateEntity> Templates => Set<TemplateEntity>();
    public DbSet<TemplateFieldEntity> TemplateFields => Set<TemplateFieldEntity>();
    public DbSet<WorkflowStateEntity> WorkflowStates => Set<WorkflowStateEntity>();
    public DbSet<WorkflowActionEntity> WorkflowActions => Set<WorkflowActionEntity>();
    public DbSet<WorkflowActionRoleEntity> WorkflowActionRoles => Set<WorkflowActionRoleEntity>();
    public DbSet<ContentItemEntity> ContentItems => Set<ContentItemEntity>();
    public DbSet<ContentFieldValueEntity> ContentFieldValues => Set<ContentFieldValueEntity>();
    public DbSet<RenderingDefinitionEntity> RenderingDefinitions => Set<RenderingDefinitionEntity>();
    public DbSet<RenderingFieldEntity> RenderingFields => Set<RenderingFieldEntity>();
    public DbSet<RenderingAllowedPlaceholderEntity> RenderingAllowedPlaceholders => Set<RenderingAllowedPlaceholderEntity>();
    public DbSet<PageRenderingEntity> PageRenderings => Set<PageRenderingEntity>();

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<TemplateEntity>(entity =>
        {
            entity.ToTable("cms_templates");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => item.TemplateKey).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new TemplateId.EfCoreValueConverter());
            entity.Property(item => item.TemplateKey).HasColumnName("template_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.Description).HasColumnName("description");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
        });

        modelBuilder.Entity<TemplateFieldEntity>(entity =>
        {
            entity.ToTable("cms_template_fields");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => new { item.TemplateId, item.FieldKey }).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new TemplateFieldId.EfCoreValueConverter());
            entity.Property(item => item.TemplateId).HasColumnName("template_id").HasMaxLength(36).HasConversion(new TemplateId.EfCoreValueConverter());
            entity.Property(item => item.FieldKey).HasColumnName("field_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.FieldType).HasColumnName("field_type").HasMaxLength(64);
            entity.Property(item => item.IsRequired).HasColumnName("is_required");
            entity.Property(item => item.SortOrder).HasColumnName("sort_order");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.Template)
                .WithMany(template => template.Fields)
                .HasForeignKey(item => item.TemplateId);
        });

        modelBuilder.Entity<WorkflowStateEntity>(entity =>
        {
            entity.ToTable("cms_workflow_states");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => item.StateKey).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new WorkflowStateId.EfCoreValueConverter());
            entity.Property(item => item.StateKey).HasColumnName("state_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.AllowsEditing).HasColumnName("allows_editing");
            entity.Property(item => item.AllowsPublishing).HasColumnName("allows_publishing");
            entity.Property(item => item.SortOrder).HasColumnName("sort_order");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
        });

        modelBuilder.Entity<WorkflowActionEntity>(entity =>
        {
            entity.ToTable("cms_workflow_actions");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => item.ActionKey).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new WorkflowActionId.EfCoreValueConverter());
            entity.Property(item => item.ActionKey).HasColumnName("action_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.FromStateId).HasColumnName("from_state_id").HasMaxLength(36).HasConversion(new WorkflowStateId.EfCoreValueConverter());
            entity.Property(item => item.ToStateId).HasColumnName("to_state_id").HasMaxLength(36).HasConversion(new WorkflowStateId.EfCoreValueConverter());
            entity.Property(item => item.SortOrder).HasColumnName("sort_order");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.FromState)
                .WithMany()
                .HasForeignKey(item => item.FromStateId);
            entity
                .HasOne(item => item.ToState)
                .WithMany()
                .HasForeignKey(item => item.ToStateId);
        });

        modelBuilder.Entity<WorkflowActionRoleEntity>(entity =>
        {
            entity.ToTable("cms_workflow_action_roles");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => new { item.WorkflowActionId, item.RoleKey }).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new WorkflowActionRoleId.EfCoreValueConverter());
            entity.Property(item => item.WorkflowActionId).HasColumnName("workflow_action_id").HasMaxLength(36).HasConversion(new WorkflowActionId.EfCoreValueConverter());
            entity.Property(item => item.RoleKey).HasColumnName("role_key").HasMaxLength(128);
            entity
                .HasOne(item => item.WorkflowAction)
                .WithMany(action => action.Roles)
                .HasForeignKey(item => item.WorkflowActionId);
        });

        modelBuilder.Entity<ContentItemEntity>(entity =>
        {
            entity.ToTable("cms_content_items");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => item.Path).IsUnique();
            entity.HasIndex(item => new { item.ParentId, item.SortOrder });
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new ContentItemId.EfCoreValueConverter());
            entity.Property(item => item.ParentId).HasColumnName("parent_id").HasMaxLength(36).HasConversion(new ContentItemId.EfCoreValueConverter());
            entity.Property(item => item.ItemKey).HasColumnName("item_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.Path).HasColumnName("path").HasMaxLength(512);
            entity.Property(item => item.TemplateId).HasColumnName("template_id").HasMaxLength(36).HasConversion(new TemplateId.EfCoreValueConverter());
            entity.Property(item => item.WorkflowStateId).HasColumnName("workflow_state_id").HasMaxLength(36).HasConversion(new WorkflowStateId.EfCoreValueConverter());
            entity.Property(item => item.SortOrder).HasColumnName("sort_order");
            entity.Property(item => item.Version).HasColumnName("version");
            entity.Property(item => item.IsDraft).HasColumnName("is_draft");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.Parent)
                .WithMany(parent => parent.Children)
                .HasForeignKey(item => item.ParentId);
            entity
                .HasOne(item => item.Template)
                .WithMany()
                .HasForeignKey(item => item.TemplateId);
            entity
                .HasOne(item => item.WorkflowState)
                .WithMany()
                .HasForeignKey(item => item.WorkflowStateId);
        });

        modelBuilder.Entity<ContentFieldValueEntity>(entity =>
        {
            entity.ToTable("cms_content_field_values");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => new { item.ContentItemId, item.FieldKey }).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new ContentFieldValueId.EfCoreValueConverter());
            entity.Property(item => item.ContentItemId).HasColumnName("content_item_id").HasMaxLength(36).HasConversion(new ContentItemId.EfCoreValueConverter());
            entity.Property(item => item.FieldKey).HasColumnName("field_key").HasMaxLength(128);
            entity.Property(item => item.FieldValue).HasColumnName("field_value");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.ContentItem)
                .WithMany(contentItem => contentItem.FieldValues)
                .HasForeignKey(item => item.ContentItemId);
        });

        modelBuilder.Entity<RenderingDefinitionEntity>(entity =>
        {
            entity.ToTable("cms_rendering_definitions");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => item.RenderingKey).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new RenderingDefinitionId.EfCoreValueConverter());
            entity.Property(item => item.RenderingKey).HasColumnName("rendering_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.ComponentKey).HasColumnName("component_key").HasMaxLength(200);
            entity.Property(item => item.DatasourceTemplateId).HasColumnName("datasource_template_id").HasMaxLength(36).HasConversion(new TemplateId.EfCoreValueConverter());
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.DatasourceTemplate)
                .WithMany()
                .HasForeignKey(item => item.DatasourceTemplateId);
        });

        modelBuilder.Entity<RenderingFieldEntity>(entity =>
        {
            entity.ToTable("cms_rendering_fields");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => new { item.RenderingDefinitionId, item.FieldKey }).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new RenderingFieldId.EfCoreValueConverter());
            entity.Property(item => item.RenderingDefinitionId).HasColumnName("rendering_definition_id").HasMaxLength(36).HasConversion(new RenderingDefinitionId.EfCoreValueConverter());
            entity.Property(item => item.FieldKey).HasColumnName("field_key").HasMaxLength(128);
            entity.Property(item => item.Name).HasColumnName("name").HasMaxLength(200);
            entity.Property(item => item.FieldType).HasColumnName("field_type").HasMaxLength(64);
            entity.Property(item => item.IsRequired).HasColumnName("is_required");
            entity.Property(item => item.SortOrder).HasColumnName("sort_order");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.RenderingDefinition)
                .WithMany(rendering => rendering.EditableFields)
                .HasForeignKey(item => item.RenderingDefinitionId);
        });

        modelBuilder.Entity<RenderingAllowedPlaceholderEntity>(entity =>
        {
            entity.ToTable("cms_rendering_allowed_placeholders");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => new { item.RenderingDefinitionId, item.PlaceholderName }).IsUnique();
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new RenderingAllowedPlaceholderId.EfCoreValueConverter());
            entity.Property(item => item.RenderingDefinitionId).HasColumnName("rendering_definition_id").HasMaxLength(36).HasConversion(new RenderingDefinitionId.EfCoreValueConverter());
            entity.Property(item => item.PlaceholderName).HasColumnName("placeholder_name").HasMaxLength(128);
            entity
                .HasOne(item => item.RenderingDefinition)
                .WithMany(rendering => rendering.AllowedPlaceholders)
                .HasForeignKey(item => item.RenderingDefinitionId);
        });

        modelBuilder.Entity<PageRenderingEntity>(entity =>
        {
            entity.ToTable("cms_page_renderings");
            entity.HasKey(item => item.Id);
            entity.HasIndex(item => new { item.ContentItemId, item.PlaceholderName, item.SortOrder });
            entity.Property(item => item.Id).HasColumnName("id").HasMaxLength(36).HasConversion(new PageRenderingId.EfCoreValueConverter());
            entity.Property(item => item.ContentItemId).HasColumnName("content_item_id").HasMaxLength(36).HasConversion(new ContentItemId.EfCoreValueConverter());
            entity.Property(item => item.RenderingDefinitionId).HasColumnName("rendering_definition_id").HasMaxLength(36).HasConversion(new RenderingDefinitionId.EfCoreValueConverter());
            entity.Property(item => item.DatasourceItemId).HasColumnName("datasource_item_id").HasMaxLength(36).HasConversion(new ContentItemId.EfCoreValueConverter());
            entity.Property(item => item.PlaceholderName).HasColumnName("placeholder_name").HasMaxLength(128);
            entity.Property(item => item.SortOrder).HasColumnName("sort_order");
            entity.Property(item => item.CreatedAtUtc).HasColumnName("created_at_utc");
            entity.Property(item => item.UpdatedAtUtc).HasColumnName("updated_at_utc");
            entity
                .HasOne(item => item.ContentItem)
                .WithMany(contentItem => contentItem.PageRenderings)
                .HasForeignKey(item => item.ContentItemId);
            entity
                .HasOne(item => item.RenderingDefinition)
                .WithMany()
                .HasForeignKey(item => item.RenderingDefinitionId);
            entity
                .HasOne(item => item.DatasourceItem)
                .WithMany()
                .HasForeignKey(item => item.DatasourceItemId);
        });
    }
}
