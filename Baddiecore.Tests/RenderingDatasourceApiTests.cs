using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using Baddiecore.Data.Ids;
using Baddiecore.Models;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;
using Testcontainers.MySql;
using Xunit;

namespace Baddiecore.Tests;

public sealed class BaddiecoreApiFixture : IAsyncLifetime
{
    private readonly MySqlContainer container = new MySqlBuilder()
        .WithImage("mysql:8.4")
        .WithDatabase("baddiecore_tests")
        .WithUsername("baddiecore")
        .WithPassword("baddiecore_test_password")
        .Build();

    private WebApplicationFactory<Program>? factory;

    public HttpClient Client { get; private set; } = null!;

    public async Task InitializeAsync()
    {
        await container.StartAsync();
        var scriptsPath = Path.Combine(AppContext.BaseDirectory, "db", "init");
        await container.ExecScriptAsync(await File.ReadAllTextAsync(Path.Combine(scriptsPath, "001_cms_schema.sql")));
        await container.ExecScriptAsync(await File.ReadAllTextAsync(Path.Combine(scriptsPath, "002_demo_content.sql")));

        factory = new WebApplicationFactory<Program>().WithWebHostBuilder(builder =>
        {
            builder.UseEnvironment("Testing");
            builder.ConfigureAppConfiguration((_, configuration) =>
            {
                configuration.AddInMemoryCollection(new Dictionary<string, string?>
                {
                    ["ConnectionStrings:Baddiecore"] = container.GetConnectionString()
                });
            });
        });
        Client = factory.CreateClient();
    }

    public Task ExecuteScriptAsync(string script) => container.ExecScriptAsync(script);

    public async Task DisposeAsync()
    {
        Client.Dispose();
        if (factory is not null)
        {
            await factory.DisposeAsync();
        }
        await container.DisposeAsync();
    }
}

public sealed class RenderingDatasourceApiTests(BaddiecoreApiFixture fixture)
    : IClassFixture<BaddiecoreApiFixture>
{
    [Fact]
    public async Task Layout_exposes_datasource_version_and_update_persists_fields()
    {
        var layout = await GetHomeLayout();
        var hero = FindRendering(layout, "hero");
        var changedHeading = $"Persisted heading {Guid.NewGuid():N}";
        var request = CreateRequest(hero, ("heading", changedHeading));

        var response = await UpdateAsync(hero, request);

        Assert.Equal(HttpStatusCode.NoContent, response.StatusCode);

        var refreshedLayout = await GetHomeLayout();
        var refreshedHero = FindRendering(refreshedLayout, "hero");
        Assert.Equal(hero.DatasourceVersion + 1, refreshedHero.DatasourceVersion);
        Assert.Equal(changedHeading, refreshedHero.Fields.Single(field => field.Name == "heading").Value);

        var staleResponse = await UpdateAsync(hero, request);

        Assert.Equal(HttpStatusCode.Conflict, staleResponse.StatusCode);
        using var conflict = JsonDocument.Parse(await staleResponse.Content.ReadAsStringAsync());
        Assert.Equal("Datasource version conflict", conflict.RootElement.GetProperty("title").GetString());
        Assert.Equal(refreshedHero.DatasourceVersion, conflict.RootElement.GetProperty("currentVersion").GetInt32());
    }

    [Fact]
    public async Task Update_rejects_invalid_datasource_unknown_fields_and_missing_required_values()
    {
        var hero = FindRendering(await GetHomeLayout(), "hero");

        var invalidIdResponse = await fixture.Client.PutAsJsonAsync(
            $"/api/renderings/{hero.Id}/datasource/fields",
            new
            {
                DatasourceId = "not-a-guid",
                ExpectedVersion = hero.DatasourceVersion,
                Fields = new[] { new { Name = "heading", Value = "Valid value" } }
            });
        Assert.Equal(HttpStatusCode.BadRequest, invalidIdResponse.StatusCode);

        var invalidVersionResponse = await fixture.Client.PutAsJsonAsync(
            $"/api/renderings/{hero.Id}/datasource/fields",
            new UpdateRenderingDatasourceRequest
            {
                DatasourceId = ContentItemId.From(Guid.Parse(hero.DatasourceId)),
                ExpectedVersion = 0,
                Fields = [new UpdateFieldValueRequest { Name = "heading", Value = "Valid value" }]
            });
        Assert.Equal(HttpStatusCode.BadRequest, invalidVersionResponse.StatusCode);

        var invalidRenderingResponse = await fixture.Client.PutAsJsonAsync(
            "/api/renderings/not-a-guid/datasource/fields",
            CreateRequest(hero));
        Assert.Equal(HttpStatusCode.BadRequest, invalidRenderingResponse.StatusCode);

        var wrongDatasourceRequest = new UpdateRenderingDatasourceRequest
        {
            DatasourceId = ContentItemId.From(Guid.NewGuid()),
            ExpectedVersion = hero.DatasourceVersion,
            Fields = CreateRequest(hero).Fields
        };
        var wrongDatasourceResponse = await UpdateAsync(hero, wrongDatasourceRequest);
        Assert.Equal(HttpStatusCode.BadRequest, wrongDatasourceResponse.StatusCode);

        var unknownFieldRequest = CreateRequest(hero);
        unknownFieldRequest = new UpdateRenderingDatasourceRequest
        {
            DatasourceId = unknownFieldRequest.DatasourceId,
            ExpectedVersion = unknownFieldRequest.ExpectedVersion,
            Fields = [.. unknownFieldRequest.Fields, new UpdateFieldValueRequest { Name = "secretField", Value = "nope" }]
        };
        var unknownFieldResponse = await UpdateAsync(hero, unknownFieldRequest);
        Assert.Equal(HttpStatusCode.BadRequest, unknownFieldResponse.StatusCode);

        var requiredRequest = CreateRequest(hero, ("heading", "   "));
        var requiredResponse = await UpdateAsync(hero, requiredRequest);
        Assert.Equal(HttpStatusCode.BadRequest, requiredResponse.StatusCode);
        Assert.Contains("A value is required", await requiredResponse.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task Update_rejects_field_types_the_editor_does_not_support()
    {
        var hero = FindRendering(await GetHomeLayout(), "hero");
        await fixture.ExecuteScriptAsync("""
            UPDATE cms_rendering_fields rf
            JOIN cms_rendering_definitions rd ON rd.id = rf.rendering_definition_id
            SET rf.field_type = 'number'
            WHERE rd.rendering_key = 'hero' AND rf.field_key = 'eyebrow';
            """);

        try
        {
            var response = await UpdateAsync(hero, CreateRequest(hero));

            Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
            Assert.Contains("not supported for editing", await response.Content.ReadAsStringAsync());
        }
        finally
        {
            await fixture.ExecuteScriptAsync("""
                UPDATE cms_rendering_fields rf
                JOIN cms_rendering_definitions rd ON rd.id = rf.rendering_definition_id
                SET rf.field_type = 'text'
                WHERE rd.rendering_key = 'hero' AND rf.field_key = 'eyebrow';
                """);
        }
    }

    private async Task<LayoutResponse> GetHomeLayout()
    {
        var layout = await fixture.Client.GetFromJsonAsync<LayoutResponse>(
            "/api/layout?path=%2F&mode=preview");
        return Assert.IsType<LayoutResponse>(layout);
    }

    private static RenderingInstanceResponse FindRendering(LayoutResponse layout, string componentKey) =>
        layout.Placeholders
            .SelectMany(placeholder => placeholder.Renderings)
            .Single(rendering => rendering.ComponentKey == componentKey);

    private static UpdateRenderingDatasourceRequest CreateRequest(
        RenderingInstanceResponse rendering,
        params (string Name, string Value)[] replacements)
    {
        var replacementValues = replacements.ToDictionary(item => item.Name, item => item.Value);
        return new UpdateRenderingDatasourceRequest
        {
            DatasourceId = ContentItemId.From(Guid.Parse(rendering.DatasourceId)),
            ExpectedVersion = rendering.DatasourceVersion,
            Fields = rendering.Fields
                .Select(field => new UpdateFieldValueRequest
                {
                    Name = field.Name,
                    Value = replacementValues.GetValueOrDefault(field.Name, field.Value)
                })
                .ToList()
        };
    }

    private Task<HttpResponseMessage> UpdateAsync(
        RenderingInstanceResponse rendering,
        UpdateRenderingDatasourceRequest request) =>
        fixture.Client.PutAsJsonAsync($"/api/renderings/{rendering.Id}/datasource/fields", request);
}
