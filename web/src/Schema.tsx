// Supertag configuration (spec 03 §5) and the field definition page (03 §6, §8): every control
// runs a command; the panel reads the `schema` snapshot again after each.

import { useEffect, useState, type ReactNode } from 'react'
import { call, type Json } from './engine'
import { Icon } from './icons'
import { tagStyle } from './Outline'
import { exec } from './controller'
import { openPicker } from './picker'
import { useStore } from './store'

function useSchema(panel: string, node: number) {
  const version = useStore((s) => s.pages[panel]?.version)
  const [d, setD] = useState<Json>(null)
  useEffect(() => {
    void call('schema', { node }).then(setD)
  }, [node, version])
  return d
}

const FIELD_TYPES: [string, string][] = [
  ['plain', 'Plain'],
  ['options', 'Options'],
  ['options_from_tag', 'Options from supertag'],
  ['date', 'Date'],
  ['number', 'Number'],
  ['url', 'URL'],
  ['email', 'E-mail'],
  ['checkbox', 'Checkbox'],
  ['user', 'User'],
]

const HIDE: [string, string][] = [
  ['never', 'Never'],
  ['when_empty', 'When empty'],
  ['when_not_empty', 'When not empty'],
  ['when_default', 'When value is default'],
  ['always', 'Always'],
]

function Select({ value, options, onChange, label }: { value: string; options: [string, string][]; onChange: (v: string) => void; label: string }) {
  return (
    <select className="select" aria-label={label} value={value} onChange={(e) => onChange(e.target.value)}>
      {options.map(([v, l]) => (
        <option key={v} value={v}>
          {l}
        </option>
      ))}
    </select>
  )
}

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button className={`toggle${on ? ' on' : ''}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  )
}

function Row({ label, help, children }: { label: string; help?: string; children: ReactNode }) {
  return (
    <div className="cfg-row">
      <div className="cfg-label">{label}</div>
      <div className="cfg-control">
        {children}
        {help && <div className="cfg-help">{help}</div>}
      </div>
    </div>
  )
}

function TextBox({ value, placeholder, onCommit, label }: { value: string; placeholder: string; onCommit: (v: string) => void; label: string }) {
  const [v, setV] = useState(value)
  useEffect(() => setV(value), [value])
  return (
    <input
      className="cfg-input"
      aria-label={label}
      value={v}
      placeholder={placeholder}
      onChange={(e) => setV(e.target.value)}
      onBlur={() => v !== value && onCommit(v)}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
      }}
    />
  )
}

// ---- field page ----------------------------------------------------------------------------------

/** Command buttons on a supertag's nodes or a field's rows (UC-06.33): the ones shown, and a menu
 * of the other command nodes to add. */
function CommandButtonsRow({ d, on, panel }: { d: Json; on: { tag: number } | { field: number }; panel: string }) {
  const shown: { id: number; name: string }[] = d.buttons ?? []
  const rest: { id: number; name: string }[] = (d.commandNodes ?? []).filter((c: Json) => !shown.some((b) => b.id === c.id))
  return (
    <Row label="Command buttons" help={'tag' in on ? 'Shown on every node with this supertag; a click runs it on the node.' : 'Shown on the field; a click runs it on the node the field is on.'}>
      <div className="cfg-options">
        {shown.map((b) => (
          <span key={b.id} className="cfg-option">
            {b.name}
            <button className="q-x" title="Remove button" onClick={() => void exec('commandNode.button', { command: b.id, ...on, off: true }, panel)}>
              <Icon name="close" size={9} />
            </button>
          </span>
        ))}
        <Select
          label="Add a command button"
          value=""
          options={[['', rest.length ? 'Add a button…' : 'No command nodes'], ...rest.map((c) => [String(c.id), c.name] as [string, string])]}
          onChange={(v) => v && void exec('commandNode.button', { command: Number(v), ...on }, panel)}
        />
      </div>
    </Row>
  )
}

export function FieldPage({ panel, field }: { panel: string; field: number }) {
  const d = useSchema(panel, field)
  const [opt, setOpt] = useState('')
  if (!d || d.kind !== 'field') return null
  const run = (c: string, p: Json) => void exec(c, { field, ...p }, panel)
  return (
    <div className="cfg-grid" aria-label="Field settings">
      <Row label="Field type" help="What a value of this field is.">
        <Select label="Field type" value={d.type} options={FIELD_TYPES} onChange={(type) => run('field.setType', { type })} />
      </Row>
      {d.type === 'options' && (
        <Row label="Pre-determined options" help="Picked from a menu when the value is typed.">
          <div className="cfg-options">
            {d.ownOptions.map((o: string) => (
              <span key={o} className="cfg-option">
                <span className="dot small" />
                {o}
                <button className="q-x" title="Remove option" onClick={() => run('field.removeOption', { option: o })}>
                  <Icon name="close" size={9} />
                </button>
              </span>
            ))}
            <input
              className="cfg-input"
              placeholder="Add an option"
              value={opt}
              onChange={(e) => setOpt(e.target.value)}
              onKeyDown={(e) => {
                e.stopPropagation()
                if (e.key === 'Enter' && opt.trim()) {
                  run('field.addOption', { option: opt.trim() })
                  setOpt('')
                }
              }}
            />
          </div>
        </Row>
      )}
      {d.type === 'options' && (
        <Row label="Sources of options" help="List of references and search nodes, whose children will become options.">
          <div className="cfg-options">
            {(d.sources as { id: number; name: string }[]).map((n) => (
              <span key={n.id} className="cfg-option">
                <span className="dot small" />
                {n.name}
                <button className="q-x" title="Remove source" onClick={() => run('field.removeSource', { node: n.id })}>
                  <Icon name="close" size={9} />
                </button>
              </span>
            ))}
            <button
              className="ghost-btn"
              onClick={(e) => {
                const r = e.currentTarget.getBoundingClientRect()
                openPicker({ mode: 'source', panel, node: field, at: -1, x: r.left, y: r.bottom })
              }}
            >
              + Add source
            </button>
          </div>
        </Row>
      )}
      {d.type === 'options' && (
        <Row label="Auto-collect values" help="New values typed in become options.">
          <Toggle label="Auto-collect values" on={d.autoCollect} onChange={(on) => run('field.setAutoCollect', { on })} />
        </Row>
      )}
      {d.type === 'options_from_tag' && (
        <Row label="Supertag" help="Nodes with this supertag are the options.">
          <button
            className="btn regular"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect()
              openPicker({ mode: 'sourceTag', panel, node: field, at: -1, x: r.left, y: r.bottom })
            }}
          >
            {d.sourceTag ? `#${d.sourceTag.name}` : 'Choose a supertag'}
          </button>
        </Row>
      )}
      {d.type === 'number' && (
        <Row label="Range" help="Values outside it get a warning.">
          <span className="cfg-pair">
            <TextBox label="Minimum" value={d.min ?? ''} placeholder="Min" onCommit={(v) => run('field.setRange', { min: v === '' ? null : Number(v), max: d.max })} />
            <TextBox label="Maximum" value={d.max ?? ''} placeholder="Max" onCommit={(v) => run('field.setRange', { min: d.min, max: v === '' ? null : Number(v) })} />
          </span>
        </Row>
      )}
      <Row label="Auto-initialize" help="How the value fills itself when a supertag adds the field.">
        <Select
          label="Auto-initialize"
          value={d.autoInit}
          options={[
            ['none', 'Off'],
            ['current_date', 'To the current date'],
            ['ancestor_day', 'To the day it is under'],
            ['ancestor_value', 'From the nearest ancestor with this field'],
          ]}
          onChange={(mode) => run('field.setAutoInit', { mode })}
        />
      </Row>
      <Row label="Default value">
        <TextBox label="Default value" value={d.default ?? ''} placeholder="None" onCommit={(value) => run('field.setDefault', { value })} />
      </Row>
      <Row label="Required" help="An empty value shows a warning.">
        <Toggle label="Required" on={d.required} onChange={(on) => run('field.setRequired', { on })} />
      </Row>
      <Row label="Hide field">
        <Select label="Hide field" value={d.hide} options={HIDE} onChange={(mode) => run('field.setHide', { mode })} />
      </Row>
      <CommandButtonsRow d={d} on={{ field }} panel={panel} />
      <button className="ghost-btn" onClick={(e) => mergeFrom(e, field, panel)}>
        <Icon name="duplicate" size={12} /> Merge into…
      </button>
      <p className="cfg-help">
        Used on {d.usedIn.nodes} {d.usedIn.nodes === 1 ? 'node' : 'nodes'} and in {d.usedIn.tags} {d.usedIn.tags === 1 ? 'supertag' : 'supertags'}.
      </p>
    </div>
  )
}

/** Merge into (UC-06.34): pick the supertag or field to keep. */
function mergeFrom(e: React.MouseEvent, node: number, panel: string) {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
  openPicker({ mode: 'mergeInto', panel, node, at: -1, x: r.left, y: r.bottom })
}

// ---- configure supertag ----------------------------------------------------------------------------

function Section({ icon, title, help, children }: { icon: 'fieldText' | 'duplicate' | 'check' | 'hash' | 'blocks'; title: string; help: string; children: ReactNode }) {
  const [open, setOpen] = useState(false) // Sections start folded (N59.32).
  return (
    <section className="cfg-card">
      <button className="cfg-card-head" onClick={() => setOpen((o) => !o)}>
        <Icon name={icon} size={18} />
        <span className="grow">
          <span className="cfg-card-title">{title}</span>
          <span className="cfg-card-help">{help}</span>
        </span>
        <Icon name={open ? 'chevronDown' : 'chevronRight'} size={14} />
      </button>
      {open && <div className="cfg-card-body">{children}</div>}
    </section>
  )
}

export function ConfigureTag({ panel, tag }: { panel: string; tag: number }) {
  const d = useSchema(panel, tag)
  if (!d || d.kind !== 'tag') return null
  const run = (c: string, p: Json) => void exec(c, { tag, ...p }, panel)
  const pickAt = (e: React.MouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    return { panel, at: -1, x: r.left, y: r.bottom }
  }
  return (
    <div className="configure" aria-label="Configure supertag">
      <div className="swatches" role="radiogroup" aria-label="Colour">
        {d.colors.map((c: string) => (
          <button
            key={c}
            className={`swatch${d.color === c ? ' on' : ''}${c === 'none' ? ' none' : ''}`}
            style={tagStyle(c === 'none' ? 'grey' : c)}
            title={c}
            role="radio"
            aria-checked={d.color === c}
            onClick={() => run('tag.setColor', { color: c })}
          >
            {c === 'none' && <Icon name="noColor" size={14} />}
          </button>
        ))}
        {/* Any other colour (N59.32): the system's colour picker behind a rainbow swatch. */}
        <label className={`swatch custom${/^#/.test(d.color ?? '') ? ' on' : ''}`} title="Custom colour" style={/^#/.test(d.color ?? '') ? tagStyle(d.color) : undefined}>
          <input type="color" aria-label="Custom colour" value={/^#/.test(d.color ?? '') ? d.color : '#888888'} onChange={(e) => run('tag.setColor', { color: e.target.value })} />
        </label>
      </div>
      {/* Commands on events (UC-02.20): a command node runs when the tag is added or a tagged
          node is checked. */}
      <Section icon="check" title="Commands" help="Command nodes that run on events or from buttons on the node.">
        {(['added', 'done'] as const).map((ev) => {
          const current = ev === 'added' ? d.onAdded : d.onDone
          return (
            <Row key={ev} label={ev === 'added' ? 'When the tag is added' : 'When checked done'}>
              <Select
                label={ev === 'added' ? 'When the tag is added' : 'When checked done'}
                value={current == null ? '' : String(current)}
                options={[['', 'Nothing'], ...d.commandNodes.map((c: Json) => [String(c.id), c.name] as [string, string])]}
                onChange={(v) => {
                  if (current != null) void exec('commandNode.on', { command: current, event: ev, tag, off: true }, panel)
                  if (v) void exec('commandNode.on', { command: Number(v), event: ev, tag }, panel)
                }}
              />
            </Row>
          )
        })}
        <CommandButtonsRow d={d} on={{ tag }} panel={panel} />
      </Section>
      <Section icon="blocks" title="Building blocks" help="What this supertag is and builds on.">
        <Row label="Base type" help="A built-in meaning (task, meeting, person…).">
          <Select label="Base type" value={d.baseType ?? ''} options={[['', 'None'], ...d.baseTypes.map((b: string) => [b, b[0].toUpperCase() + b.slice(1)] as [string, string])]} onChange={(base) => run('tag.setBaseType', { base: base || null })} />
        </Row>
        <Row label="Extends" help="Its fields come first on this supertag's nodes.">
          <div className="cfg-options">
            {d.extends.map((t: Json) => (
              <span key={t.id} className="cfg-option">
                #{t.name}
              </span>
            ))}
          </div>
        </Row>
        <Row label="Show a checkbox" help="Nodes with this supertag can be checked off.">
          <Toggle label="Show a checkbox" on={d.checkbox} onChange={(on) => run('tag.setCheckbox', { on })} />
        </Row>
      </Section>
      <Section icon="fieldText" title="Content template" help="Fields every node with this supertag gets.">
        <div className="cfg-fields">
          {d.fields.map((f: Json) => (
            <div key={f.id} className="cfg-field">
              <Icon name="fieldText" size={13} />
              <span className="grow" onClick={() => void exec('nav.open', { node: f.id }, panel)}>
                {f.name}
              </span>
              <span className="cfg-help">{f.type}</span>
              {f.own && (
                <button className="q-x" title="Remove from template" onClick={() => run('tag.removeField', { field: f.id })}>
                  <Icon name="close" size={9} />
                </button>
              )}
            </div>
          ))}
          <button className="ghost-btn" onClick={(e) => openPicker({ ...pickAt(e), mode: 'templateField', node: tag })}>
            + Add field
          </button>
        </div>
      </Section>
      <Section icon="hash" title="Advanced" help="Titles from fields and the supertag for new children.">
        <Row label="Build title from fields" help="Start with ${name}; ${Field} inserts a field's value.">
          <TextBox label="Title expression" value={d.titleExpr ?? ''} placeholder="${name}" onCommit={(expression) => run('tag.setTitleExpression', { expression })} />
        </Row>
      </Section>
      {/* Footer: how often the tag is used, and Delete (spec 12 d46). */}
      <div className="cfg-foot">
        <span className="cfg-used">
          <Icon name="list" size={12} /> Used {d.used} {d.used === 1 ? 'time' : 'times'}
        </span>
        {/* A duplicate supertag goes into another (UC-06.34). */}
        <button className="ghost-btn" onClick={(e) => mergeFrom(e, tag, panel)}>
          <Icon name="duplicate" size={12} /> Merge into…
        </button>
        <button className="ghost-btn danger" onClick={() => void exec('node.delete', { node: tag }, panel)}>
          <Icon name="trash" size={12} /> Delete
        </button>
      </div>
    </div>
  )
}
