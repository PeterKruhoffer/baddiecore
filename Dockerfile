# syntax=docker/dockerfile:1.7

FROM node:24-alpine AS cms-dashboard-build
WORKDIR /src/cms-dashboard

RUN corepack enable

COPY cms-dashboard/package.json cms-dashboard/pnpm-lock.yaml cms-dashboard/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

COPY cms-dashboard/ ./
RUN pnpm run build

FROM mcr.microsoft.com/dotnet/sdk:10.0-alpine AS build
WORKDIR /src

COPY Baddiecore.csproj ./
RUN dotnet restore Baddiecore.csproj

COPY . ./
RUN dotnet publish Baddiecore.csproj \
    --configuration Release \
    --output /app/publish \
    /p:UseAppHost=false

COPY --from=cms-dashboard-build /src/wwwroot/cms /app/publish/wwwroot/cms

FROM mcr.microsoft.com/dotnet/aspnet:10.0-alpine AS runtime
WORKDIR /app

ENV DOTNET_EnableDiagnostics=0

EXPOSE 8080

COPY --from=build /app/publish ./
USER $APP_UID
ENTRYPOINT ["dotnet", "Baddiecore.dll"]
