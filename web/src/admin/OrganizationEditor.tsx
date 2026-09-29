import { createSignal, For, Show } from "solid-js";
import * as stylex from "@stylexjs/stylex";
import type { Group, Member, Organization, Role } from "../types";
import { api } from "../lib/api";
import { createRequest } from "../lib/resource";
import { common } from "../common.stylex";

const styles = stylex.create({
  layout: { padding: 28, maxWidth: 1000, margin: "auto" },
  card: {
    padding: 20,
    marginBottom: 18,
    backgroundColor: "#fffdf8",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "#ddd6cb",
    borderRadius: 12,
  },
  row: { display: "flex", gap: 12, flexWrap: "wrap" },
});
const paths = (value: FormDataEntryValue | null) =>
  String(value || "")
    .split(/[\s,]+/)
    .filter(Boolean);

export function OrganizationEditor(p: {
  onDirty: (value: boolean) => void;
  onRefresh: () => Promise<void>;
}) {
  const request = createRequest(api.organization);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function save(org: Organization) {
    setBusy(true);
    setError("");
    try {
      await api.saveOrganization(org);
      p.onDirty(false);
      await request.refetch();
      await p.onRefresh();
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not save membership");
      return false;
    } finally {
      setBusy(false);
    }
  }
  function memberForm(member?: Member) {
    return (
      <form
        {...stylex.attrs(styles.card)}
        onInput={() => p.onDirty(true)}
        onSubmit={(e) => {
          e.preventDefault();
          const form = e.currentTarget;
          const data = new FormData(form),
            org = request.value()!;
          const next: Member = {
            id: String(data.get("id")),
            name: String(data.get("name")),
            role: String(data.get("role")) as Role,
            paths: paths(data.get("paths")),
            groups: data.getAll("groups").map(String),
          };
          void save({
            ...org,
            members: [...org.members.filter((m) => m.id !== member?.id), next],
          }).then((saved) => {
            if (saved && !member) form.reset();
          });
        }}
      >
        <h3>{member ? member.name : "Add member"}</h3>
        <div {...stylex.attrs(styles.row)}>
          <label {...stylex.attrs(common.label)}>
            Provider user ID
            <input
              {...stylex.attrs(common.control)}
              name="id"
              required
              value={member?.id || ""}
              readonly={!!member}
            />
          </label>
          <label {...stylex.attrs(common.label)}>
            Display name
            <input
              {...stylex.attrs(common.control)}
              name="name"
              required
              value={member?.name || ""}
            />
          </label>
          <label {...stylex.attrs(common.label)}>
            Role
            <select {...stylex.attrs(common.control)} name="role" value={member?.role || "editor"}>
              <option value="editor">Editor</option>
              <option value="reviewer">Reviewer / super user</option>
              <option value="admin">Administrator</option>
            </select>
          </label>
        </div>
        <label {...stylex.attrs(common.label)}>
          Individual path grants, separated by commas
          <input
            {...stylex.attrs(common.control)}
            name="paths"
            placeholder="/news, /about/team"
            value={member?.paths.join(", ") || ""}
          />
        </label>
        <fieldset>
          <legend>Groups</legend>
          <For each={request.value()?.groups}>
            {(group) => (
              <label>
                <input
                  type="checkbox"
                  name="groups"
                  value={group.id}
                  checked={member?.groups.includes(group.id) || false}
                />{" "}
                {group.name}{" "}
              </label>
            )}
          </For>
        </fieldset>
        <button {...stylex.attrs(common.button, common.primary)} disabled={busy()}>
          Save member
        </button>
        <Show when={member}>
          <button
            type="button"
            {...stylex.attrs(common.button, common.danger)}
            disabled={busy()}
            onClick={() => {
              if (confirm(`Remove ${member!.name}'s CMS access?`)) {
                const org = request.value()!;
                void save({ ...org, members: org.members.filter((m) => m.id !== member!.id) });
              }
            }}
          >
            Remove member
          </button>
        </Show>
      </form>
    );
  }
  function groupForm(group?: Group) {
    return (
      <form
        {...stylex.attrs(styles.card)}
        onInput={() => p.onDirty(true)}
        onSubmit={(e) => {
          e.preventDefault();
          const form = e.currentTarget;
          const data = new FormData(form),
            org = request.value()!;
          const next: Group = {
            id: group?.id || crypto.randomUUID(),
            name: String(data.get("name")),
            paths: paths(data.get("paths")),
          };
          void save({
            ...org,
            groups: [...org.groups.filter((g) => g.id !== group?.id), next],
          }).then((saved) => {
            if (saved && !group) form.reset();
          });
        }}
      >
        <h3>{group ? group.name : "Add group"}</h3>
        <label {...stylex.attrs(common.label)}>
          Group name
          <input {...stylex.attrs(common.control)} name="name" required value={group?.name || ""} />
        </label>
        <label {...stylex.attrs(common.label)}>
          Path grants
          <input
            {...stylex.attrs(common.control)}
            name="paths"
            placeholder="/news, /about/team"
            value={group?.paths.join(", ") || ""}
          />
        </label>
        <button {...stylex.attrs(common.button, common.primary)} disabled={busy()}>
          Save group
        </button>
        <Show when={group}>
          <button
            type="button"
            {...stylex.attrs(common.button, common.danger)}
            disabled={busy()}
            onClick={() => {
              if (confirm(`Remove ${group!.name} and its grants?`)) {
                const org = request.value()!;
                void save({
                  ...org,
                  groups: org.groups.filter((g) => g.id !== group!.id),
                  members: org.members.map((m) => ({
                    ...m,
                    groups: m.groups.filter((id) => id !== group!.id),
                  })),
                });
              }
            }}
          >
            Remove group
          </button>
        </Show>
      </form>
    );
  }
  return (
    <section {...stylex.attrs(styles.layout)}>
      <h1>Organization</h1>
      <p>
        One CMS installation is one organization. Add existing provider user IDs, not email
        addresses. Unknown users have no access. This does not invite or change users at WorkOS.
      </p>
      <p>
        Administrators have full access. Reviewers edit all pages and approve submissions. Editors
        can edit and submit only within their individual or group paths. A grant includes its
        descendants, and stays at that path when pages move. Use / for all paths.
      </p>
      <p>Save one member or group at a time. Saving reloads the organization forms.</p>
      <Show when={error() || request.error()}>
        <p role="alert" {...stylex.attrs(common.error)}>
          {error() || String(request.error())}
        </p>
        <button {...stylex.attrs(common.button)} onClick={() => void request.refetch()}>
          Reload membership
        </button>
      </Show>
      <Show when={request.value()}>
        <div inert={busy()}>
          <h2>Members</h2>
          <For each={request.value()?.members}>{(member) => memberForm(member)}</For>
          {memberForm()}
          <h2>Groups</h2>
          <For each={request.value()?.groups}>{(group) => groupForm(group)}</For>
          {groupForm()}
        </div>
      </Show>
    </section>
  );
}
