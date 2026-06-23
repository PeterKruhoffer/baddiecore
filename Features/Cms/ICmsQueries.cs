using Baddiecore.Models;

namespace Baddiecore.Features.Cms;

public interface ICmsQueries
{
    Task<ContentTreeResponse> GetContentTreeAsync(CancellationToken cancellationToken);

    Task<LayoutResponse?> GetLayoutAsync(
        string path,
        string mode,
        CancellationToken cancellationToken);

    Task<TemplateCatalogResponse> GetTemplatesAsync(CancellationToken cancellationToken);

    Task<RenderingCatalogResponse> GetRenderingsAsync(CancellationToken cancellationToken);

    Task<WorkflowResponse> GetWorkflowAsync(CancellationToken cancellationToken);
}
