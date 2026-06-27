using Baddiecore.Data.Ids;

namespace Baddiecore.Models;

public sealed class GetLayoutRequest
{
    public string Path { get; init; } = "/";
    public string Mode { get; init; } = "preview";
}

public sealed record ContentTreeResponse(IReadOnlyList<ContentTreeNodeResponse> Items);

public sealed record ContentTreeNodeResponse(
    string Id,
    string Name,
    string Path,
    string Template,
    string WorkflowState,
    IReadOnlyList<ContentTreeNodeResponse> Children);

public sealed record LayoutResponse(
    string Path,
    string Mode,
    PageMetadataResponse Metadata,
    IReadOnlyDictionary<string, string> Fields,
    IReadOnlyList<PlaceholderResponse> Placeholders);

public sealed record PageMetadataResponse(
    string Id,
    string Name,
    string Template,
    string WorkflowState,
    int Version,
    bool IsDraft);

public sealed record PlaceholderResponse(
    string Name,
    IReadOnlyList<RenderingInstanceResponse> Renderings);

public sealed record RenderingInstanceResponse(
    string Id,
    string Name,
    string ComponentKey,
    string DatasourceId,
    int DatasourceVersion,
    IReadOnlyList<FieldValueResponse> Fields);

public sealed record FieldValueResponse(
    string Name,
    string Label,
    string Type,
    bool Required,
    string Value);

public sealed class UpdateRenderingDatasourceRequest
{
    public ContentItemId DatasourceId { get; init; } = null!;
    public int ExpectedVersion { get; init; }
    public IReadOnlyList<UpdateFieldValueRequest> Fields { get; init; } = [];
}

public sealed class UpdateFieldValueRequest
{
    public string Name { get; init; } = string.Empty;
    public string? Value { get; init; }
}

public sealed record TemplateCatalogResponse(IReadOnlyList<TemplateDefinitionResponse> Templates);

public sealed record TemplateDefinitionResponse(
    string Name,
    string DisplayName,
    IReadOnlyList<FieldDefinitionResponse> Fields);

public sealed record RenderingCatalogResponse(IReadOnlyList<RenderingDefinitionResponse> Renderings);

public sealed record RenderingDefinitionResponse(
    string Name,
    string ComponentKey,
    string DatasourceTemplate,
    IReadOnlyList<string> AllowedPlaceholders,
    IReadOnlyList<FieldDefinitionResponse> EditableFields);

public sealed record FieldDefinitionResponse(
    string Name,
    string Label,
    string Type,
    bool Required);

public sealed record WorkflowResponse(
    IReadOnlyList<WorkflowStateResponse> States,
    IReadOnlyList<WorkflowActionResponse> Actions);

public sealed record WorkflowStateResponse(
    string Name,
    bool AllowsEditing,
    bool AllowsPublishing);

public sealed record WorkflowActionResponse(
    string Name,
    string FromState,
    string ToState,
    IReadOnlyList<string> Roles);
