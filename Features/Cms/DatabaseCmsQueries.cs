using Baddiecore.Data;
using Baddiecore.Data.Entities;
using Baddiecore.Data.Ids;
using Baddiecore.Models;
using Microsoft.EntityFrameworkCore;

namespace Baddiecore.Features.Cms;

public sealed class DatabaseCmsQueries(BaddiecoreDbContext dbContext) : ICmsQueries
{
    public async Task<ContentTreeResponse> GetContentTreeAsync(CancellationToken cancellationToken)
    {
        var items = await dbContext.ContentItems
            .AsNoTracking()
            .Include(item => item.Template)
            .Include(item => item.WorkflowState)
            .OrderBy(item => item.ParentId)
            .ThenBy(item => item.SortOrder)
            .ThenBy(item => item.Name)
            .ToListAsync(cancellationToken);

        return new ContentTreeResponse(BuildContentTree(items, parentId: null));
    }

    public async Task<LayoutResponse?> GetLayoutAsync(
        string path,
        string mode,
        CancellationToken cancellationToken)
    {
        var normalizedPath = NormalizePath(path);
        var normalizedMode = CmsLayoutModes.Normalize(mode);
        var page = await dbContext.ContentItems
            .AsNoTracking()
            .Include(item => item.Template)
            .Include(item => item.WorkflowState)
            .FirstOrDefaultAsync(item => item.Path == normalizedPath, cancellationToken);

        if (page is null)
        {
            return null;
        }

        var fields = await dbContext.ContentFieldValues
            .AsNoTracking()
            .Where(field => field.ContentItemId == page.Id)
            .OrderBy(field => field.FieldKey)
            .ToDictionaryAsync(
                field => field.FieldKey,
                field => field.FieldValue ?? string.Empty,
                cancellationToken);

        var pageRenderings = await dbContext.PageRenderings
            .AsNoTracking()
            .Where(rendering => rendering.ContentItemId == page.Id)
            .Include(rendering => rendering.RenderingDefinition)
                .ThenInclude(definition => definition!.EditableFields)
            .Include(rendering => rendering.DatasourceItem)
                .ThenInclude(datasource => datasource!.FieldValues)
            .AsSplitQuery()
            .OrderBy(rendering => rendering.PlaceholderName)
            .ThenBy(rendering => rendering.SortOrder)
            .ToListAsync(cancellationToken);

        var placeholders = pageRenderings
            .GroupBy(rendering => rendering.PlaceholderName)
            .Select(group => new PlaceholderResponse(
                group.Key,
                group.Select(ToRenderingInstance).ToList()))
            .ToList();

        return new LayoutResponse(
            page.Path,
            normalizedMode,
            new PageMetadataResponse(
                page.Id.Value,
                page.Name,
                page.Template?.TemplateKey ?? string.Empty,
                page.WorkflowState?.Name ?? string.Empty,
                page.Version,
                page.IsDraft),
            fields,
            placeholders);
    }

    public async Task<TemplateCatalogResponse> GetTemplatesAsync(CancellationToken cancellationToken)
    {
        var templates = await dbContext.Templates
            .AsNoTracking()
            .Include(template => template.Fields)
            .OrderBy(template => template.Name)
            .ToListAsync(cancellationToken);

        return new TemplateCatalogResponse(
            templates
                .Select(template => new TemplateDefinitionResponse(
                    template.TemplateKey,
                    template.Name,
                    template.Fields
                        .OrderBy(field => field.SortOrder)
                        .ThenBy(field => field.Name)
                        .Select(ToFieldDefinition)
                        .ToList()))
                .ToList());
    }

    public async Task<RenderingCatalogResponse> GetRenderingsAsync(CancellationToken cancellationToken)
    {
        var renderings = await dbContext.RenderingDefinitions
            .AsNoTracking()
            .Include(rendering => rendering.DatasourceTemplate)
            .Include(rendering => rendering.AllowedPlaceholders)
            .Include(rendering => rendering.EditableFields)
            .AsSplitQuery()
            .OrderBy(rendering => rendering.Name)
            .ToListAsync(cancellationToken);

        return new RenderingCatalogResponse(
            renderings
                .Select(rendering => new RenderingDefinitionResponse(
                    rendering.Name,
                    rendering.ComponentKey,
                    rendering.DatasourceTemplate?.TemplateKey ?? string.Empty,
                    rendering.AllowedPlaceholders
                        .OrderBy(placeholder => placeholder.PlaceholderName)
                        .Select(placeholder => placeholder.PlaceholderName)
                        .ToList(),
                    rendering.EditableFields
                        .OrderBy(field => field.SortOrder)
                        .ThenBy(field => field.Name)
                        .Select(ToFieldDefinition)
                        .ToList()))
                .ToList());
    }

    public async Task<WorkflowResponse> GetWorkflowAsync(CancellationToken cancellationToken)
    {
        var states = await dbContext.WorkflowStates
            .AsNoTracking()
            .OrderBy(state => state.SortOrder)
            .ThenBy(state => state.Name)
            .ToListAsync(cancellationToken);

        var actions = await dbContext.WorkflowActions
            .AsNoTracking()
            .Include(action => action.FromState)
            .Include(action => action.ToState)
            .Include(action => action.Roles)
            .OrderBy(action => action.SortOrder)
            .ThenBy(action => action.Name)
            .ToListAsync(cancellationToken);

        return new WorkflowResponse(
            states
                .Select(state => new WorkflowStateResponse(
                    state.Name,
                    state.AllowsEditing,
                    state.AllowsPublishing))
                .ToList(),
            actions
                .Select(action => new WorkflowActionResponse(
                    action.Name,
                    action.FromState?.Name ?? string.Empty,
                    action.ToState?.Name ?? string.Empty,
                    action.Roles
                        .OrderBy(role => role.RoleKey)
                        .Select(role => role.RoleKey)
                        .ToList()))
                .ToList());
    }

    private static IReadOnlyList<ContentTreeNodeResponse> BuildContentTree(
        IReadOnlyList<ContentItemEntity> items,
        ContentItemId? parentId)
    {
        return items
            .Where(item => item.ParentId == parentId)
            .Select(item => new ContentTreeNodeResponse(
                item.Id.Value,
                item.Name,
                item.Path,
                item.Template?.TemplateKey ?? string.Empty,
                item.WorkflowState?.Name ?? string.Empty,
                BuildContentTree(items, item.Id)))
            .ToList();
    }

    private static RenderingInstanceResponse ToRenderingInstance(PageRenderingEntity pageRendering)
    {
        var renderingDefinition = pageRendering.RenderingDefinition;
        var datasourceValues = pageRendering.DatasourceItem?.FieldValues
            .ToDictionary(field => field.FieldKey, field => field.FieldValue ?? string.Empty)
            ?? [];

        return new RenderingInstanceResponse(
            pageRendering.Id.Value,
            renderingDefinition?.Name ?? string.Empty,
            renderingDefinition?.ComponentKey ?? string.Empty,
            pageRendering.DatasourceItemId?.Value ?? string.Empty,
            renderingDefinition?.EditableFields
                .OrderBy(field => field.SortOrder)
                .ThenBy(field => field.Name)
                .Select(field => new FieldValueResponse(
                    field.FieldKey,
                    field.Name,
                    field.FieldType,
                    datasourceValues.GetValueOrDefault(field.FieldKey, string.Empty)))
                .ToList()
                ?? []);
    }

    private static FieldDefinitionResponse ToFieldDefinition(TemplateFieldEntity field)
    {
        return new FieldDefinitionResponse(
            field.FieldKey,
            field.Name,
            field.FieldType,
            field.IsRequired);
    }

    private static FieldDefinitionResponse ToFieldDefinition(RenderingFieldEntity field)
    {
        return new FieldDefinitionResponse(
            field.FieldKey,
            field.Name,
            field.FieldType,
            field.IsRequired);
    }

    private static string NormalizePath(string? path)
    {
        if (string.IsNullOrWhiteSpace(path) || path == "/")
        {
            return "/";
        }

        var trimmed = path.Trim();
        return trimmed.StartsWith('/') ? trimmed : $"/{trimmed}";
    }
}
