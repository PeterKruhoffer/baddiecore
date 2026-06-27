using Baddiecore.Data.Ids;
using Baddiecore.Models;

namespace Baddiecore.Features.Cms;

public interface IRenderingDatasourceWriter
{
    Task<UpdateDatasourceResult> UpdateAsync(
        PageRenderingId renderingId,
        UpdateRenderingDatasourceRequest request,
        CancellationToken cancellationToken);
}

public abstract record UpdateDatasourceResult;

public sealed record UpdateDatasourceSuccess : UpdateDatasourceResult;

public sealed record UpdateDatasourceNotFound(string Message) : UpdateDatasourceResult;

public sealed record UpdateDatasourceValidation(IReadOnlyDictionary<string, string[]> Errors) : UpdateDatasourceResult;

public sealed record UpdateDatasourceConflict(int CurrentVersion) : UpdateDatasourceResult;
