using Baddiecore.Data;
using Baddiecore.Data.Entities;
using Baddiecore.Data.Ids;
using Baddiecore.Models;
using Microsoft.EntityFrameworkCore;

namespace Baddiecore.Features.Cms;

public sealed class DatabaseRenderingDatasourceWriter(BaddiecoreDbContext dbContext)
    : IRenderingDatasourceWriter
{
    private static readonly HashSet<string> SupportedFieldTypes =
        new(StringComparer.OrdinalIgnoreCase) { "text", "textarea" };

    public async Task<UpdateDatasourceResult> UpdateAsync(
        PageRenderingId renderingId,
        UpdateRenderingDatasourceRequest request,
        CancellationToken cancellationToken)
    {
        var rendering = await dbContext.PageRenderings
            .Where(item => item.Id == renderingId)
            .Include(item => item.RenderingDefinition)
                .ThenInclude(definition => definition!.EditableFields)
            .Include(item => item.DatasourceItem)
                .ThenInclude(datasource => datasource!.FieldValues)
            .AsSplitQuery()
            .SingleOrDefaultAsync(cancellationToken);

        if (rendering is null)
        {
            return new UpdateDatasourceNotFound($"Rendering '{renderingId.Value:D}' was not found.");
        }

        if (rendering.DatasourceItem is null || rendering.DatasourceItemId is null)
        {
            return new UpdateDatasourceNotFound("The rendering does not have a datasource.");
        }

        if (rendering.DatasourceItemId != request.DatasourceId)
        {
            return Validation("datasourceId", "Datasource ID does not belong to the selected rendering.");
        }

        var definitions = rendering.RenderingDefinition?.EditableFields
            .ToDictionary(field => field.FieldKey, StringComparer.OrdinalIgnoreCase)
            ?? new Dictionary<string, RenderingFieldEntity>(StringComparer.OrdinalIgnoreCase);
        var submittedFields = request.Fields.ToDictionary(field => field.Name, StringComparer.OrdinalIgnoreCase);
        var errors = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);

        foreach (var field in request.Fields)
        {
            if (!definitions.TryGetValue(field.Name, out var definition))
            {
                errors[$"fields.{field.Name}"] = ["Field is not editable for the selected rendering."];
                continue;
            }

            if (!SupportedFieldTypes.Contains(definition.FieldType))
            {
                errors[$"fields.{field.Name}"] = [$"Field type '{definition.FieldType}' is not supported for editing."];
            }
        }

        var currentFields = rendering.DatasourceItem.FieldValues
            .ToDictionary(field => field.FieldKey, StringComparer.OrdinalIgnoreCase);

        foreach (var definition in definitions.Values.Where(field => field.IsRequired))
        {
            var value = submittedFields.TryGetValue(definition.FieldKey, out var submitted)
                ? submitted.Value
                : currentFields.GetValueOrDefault(definition.FieldKey)?.FieldValue;

            if (string.IsNullOrWhiteSpace(value))
            {
                errors[$"fields.{definition.FieldKey}"] = ["A value is required."];
            }
        }

        if (errors.Count > 0)
        {
            return new UpdateDatasourceValidation(errors);
        }

        await using var transaction = await dbContext.Database.BeginTransactionAsync(cancellationToken);
        var contentItemId = request.DatasourceId;
        var updatedItems = await dbContext.ContentItems
            .Where(item => item.Id == contentItemId && item.Version == request.ExpectedVersion)
            .ExecuteUpdateAsync(
                setters => setters
                    .SetProperty(item => item.Version, item => item.Version + 1)
                    .SetProperty(item => item.UpdatedAtUtc, DateTime.UtcNow),
                cancellationToken);

        if (updatedItems == 0)
        {
            await transaction.RollbackAsync(cancellationToken);
            var currentVersion = await dbContext.ContentItems
                .AsNoTracking()
                .Where(item => item.Id == contentItemId)
                .Select(item => item.Version)
                .SingleAsync(cancellationToken);
            return new UpdateDatasourceConflict(currentVersion);
        }

        var updatedAtUtc = DateTime.UtcNow;
        foreach (var field in request.Fields)
        {
            if (currentFields.TryGetValue(field.Name, out var currentField))
            {
                currentField.FieldValue = field.Value;
                currentField.UpdatedAtUtc = updatedAtUtc;
            }
            else
            {
                dbContext.ContentFieldValues.Add(new ContentFieldValueEntity
                {
                    Id = ContentFieldValueId.From(Guid.NewGuid()),
                    ContentItemId = contentItemId,
                    FieldKey = field.Name,
                    FieldValue = field.Value,
                    CreatedAtUtc = updatedAtUtc,
                    UpdatedAtUtc = updatedAtUtc
                });
            }
        }

        await dbContext.SaveChangesAsync(cancellationToken);
        await transaction.CommitAsync(cancellationToken);

        return new UpdateDatasourceSuccess();
    }

    private static UpdateDatasourceValidation Validation(string key, string message) =>
        new(new Dictionary<string, string[]> { [key] = [message] });
}
