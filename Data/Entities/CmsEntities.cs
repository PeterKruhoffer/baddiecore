using Baddiecore.Data.Ids;

namespace Baddiecore.Data.Entities;

public sealed class TemplateEntity
{
    public TemplateId Id { get; set; } = null!;
    public string TemplateKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public List<TemplateFieldEntity> Fields { get; } = [];
}

public sealed class TemplateFieldEntity
{
    public TemplateFieldId Id { get; set; } = null!;
    public TemplateId TemplateId { get; set; } = null!;
    public string FieldKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string FieldType { get; set; } = string.Empty;
    public bool IsRequired { get; set; }
    public int SortOrder { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public TemplateEntity? Template { get; set; }
}

public sealed class WorkflowStateEntity
{
    public WorkflowStateId Id { get; set; } = null!;
    public string StateKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public bool AllowsEditing { get; set; }
    public bool AllowsPublishing { get; set; }
    public int SortOrder { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
}

public sealed class WorkflowActionEntity
{
    public WorkflowActionId Id { get; set; } = null!;
    public string ActionKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public WorkflowStateId FromStateId { get; set; } = null!;
    public WorkflowStateId ToStateId { get; set; } = null!;
    public int SortOrder { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public WorkflowStateEntity? FromState { get; set; }
    public WorkflowStateEntity? ToState { get; set; }
    public List<WorkflowActionRoleEntity> Roles { get; } = [];
}

public sealed class WorkflowActionRoleEntity
{
    public WorkflowActionRoleId Id { get; set; } = null!;
    public WorkflowActionId WorkflowActionId { get; set; } = null!;
    public string RoleKey { get; set; } = string.Empty;
    public WorkflowActionEntity? WorkflowAction { get; set; }
}

public sealed class ContentItemEntity
{
    public ContentItemId Id { get; set; } = null!;
    public ContentItemId? ParentId { get; set; }
    public string ItemKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string Path { get; set; } = string.Empty;
    public TemplateId TemplateId { get; set; } = null!;
    public WorkflowStateId WorkflowStateId { get; set; } = null!;
    public int SortOrder { get; set; }
    public int Version { get; set; }
    public bool IsDraft { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public ContentItemEntity? Parent { get; set; }
    public TemplateEntity? Template { get; set; }
    public WorkflowStateEntity? WorkflowState { get; set; }
    public List<ContentItemEntity> Children { get; } = [];
    public List<ContentFieldValueEntity> FieldValues { get; } = [];
    public List<PageRenderingEntity> PageRenderings { get; } = [];
}

public sealed class ContentFieldValueEntity
{
    public ContentFieldValueId Id { get; set; } = null!;
    public ContentItemId ContentItemId { get; set; } = null!;
    public string FieldKey { get; set; } = string.Empty;
    public string? FieldValue { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public ContentItemEntity? ContentItem { get; set; }
}

public sealed class RenderingDefinitionEntity
{
    public RenderingDefinitionId Id { get; set; } = null!;
    public string RenderingKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string ComponentKey { get; set; } = string.Empty;
    public TemplateId? DatasourceTemplateId { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public TemplateEntity? DatasourceTemplate { get; set; }
    public List<RenderingFieldEntity> EditableFields { get; } = [];
    public List<RenderingAllowedPlaceholderEntity> AllowedPlaceholders { get; } = [];
}

public sealed class RenderingFieldEntity
{
    public RenderingFieldId Id { get; set; } = null!;
    public RenderingDefinitionId RenderingDefinitionId { get; set; } = null!;
    public string FieldKey { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string FieldType { get; set; } = string.Empty;
    public bool IsRequired { get; set; }
    public int SortOrder { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public RenderingDefinitionEntity? RenderingDefinition { get; set; }
}

public sealed class RenderingAllowedPlaceholderEntity
{
    public RenderingAllowedPlaceholderId Id { get; set; } = null!;
    public RenderingDefinitionId RenderingDefinitionId { get; set; } = null!;
    public string PlaceholderName { get; set; } = string.Empty;
    public RenderingDefinitionEntity? RenderingDefinition { get; set; }
}

public sealed class PageRenderingEntity
{
    public PageRenderingId Id { get; set; } = null!;
    public ContentItemId ContentItemId { get; set; } = null!;
    public RenderingDefinitionId RenderingDefinitionId { get; set; } = null!;
    public ContentItemId? DatasourceItemId { get; set; }
    public string PlaceholderName { get; set; } = string.Empty;
    public int SortOrder { get; set; }
    public DateTime CreatedAtUtc { get; set; }
    public DateTime UpdatedAtUtc { get; set; }
    public ContentItemEntity? ContentItem { get; set; }
    public ContentItemEntity? DatasourceItem { get; set; }
    public RenderingDefinitionEntity? RenderingDefinition { get; set; }
}
