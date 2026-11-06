// Panel de propiedades de la forma seleccionada: capa y coordenadas exactas en µm.
// Los campos aplican al presionar Enter o al salir del campo.

import { useEffect, useState } from 'react';
import { snap } from './edit.ts';
import { layerKey, type Shape, type Tech } from './model.ts';

interface Props {
  shape: Shape;
  tech: Tech;
  dbuUm: number;
  onChange: (s: Shape) => void;
}

function Num({ label, value, dbuUm, onSet }: { label: string; value: number; dbuUm: number; onSet: (dbu: number) => void }) {
  const fmt = (v: number) => (v * dbuUm).toFixed(3);
  const [text, setText] = useState(fmt(value));
  useEffect(() => setText(fmt(value)), [value, dbuUm]); // eslint-disable-line react-hooks/exhaustive-deps
  const apply = () => {
    const v = Number(text.replace(',', '.'));
    if (Number.isFinite(v)) onSet(snap({ x: v / dbuUm, y: 0 }).x);
    else setText(fmt(value));
  };
  return (
    <label className="num">
      <span>{label}</span>
      <input
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={apply}
        onKeyDown={(e) => {
          if (e.key === 'Enter') apply();
          if (e.key === 'Escape') setText(fmt(value));
        }}
      />
    </label>
  );
}

export function Properties({ shape, tech, dbuUm, onChange }: Props) {
  const key = layerKey(shape.layer);
  const known = tech.layers.some((l) => layerKey(l.layer) === key);
  const n = (label: string, value: number, onSet: (v: number) => void) => (
    <Num label={label} value={value} dbuUm={dbuUm} onSet={onSet} />
  );
  return (
    <div className="props">
      <h3>Propiedades</h3>
      <label className="num">
        <span>Capa</span>
        <select
          value={key}
          onChange={(e) => {
            const [layer, datatype] = e.target.value.split('/').map(Number);
            onChange({ ...shape, layer: { layer, datatype } });
          }}
        >
          {!known && <option value={key}>{key}</option>}
          {tech.layers.map((l) => (
            <option key={layerKey(l.layer)} value={layerKey(l.layer)}>
              {l.name} ({layerKey(l.layer)})
            </option>
          ))}
        </select>
      </label>
      {shape.kind === 'rect' &&
        (() => {
          const { p0, p1 } = shape.rect;
          const set = (a: typeof p0, b: typeof p1) =>
            onChange({ ...shape, rect: { p0: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) }, p1: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) } } });
          return (
            <div className="grid2">
              {n('x0', p0.x, (v) => set({ ...p0, x: v }, p1))}
              {n('y0', p0.y, (v) => set({ ...p0, y: v }, p1))}
              {n('x1', p1.x, (v) => set(p0, { ...p1, x: v }))}
              {n('y1', p1.y, (v) => set(p0, { ...p1, y: v }))}
              {n('Ancho', p1.x - p0.x, (v) => set(p0, { ...p1, x: p0.x + Math.max(v, 0) }))}
              {n('Alto', p1.y - p0.y, (v) => set(p0, { ...p1, y: p0.y + Math.max(v, 0) }))}
            </div>
          );
        })()}
      {shape.kind === 'path' && n('Ancho', shape.width, (v) => onChange({ ...shape, width: Math.max(v, 0) }))}
      {shape.kind === 'label' && (
        <>
          <label className="num">
            <span>Texto</span>
            <input defaultValue={shape.text} key={shape.text} onBlur={(e) => onChange({ ...shape, text: e.target.value })} />
          </label>
          <div className="grid2">
            {n('x', shape.at.x, (v) => onChange({ ...shape, at: { ...shape.at, x: v } }))}
            {n('y', shape.at.y, (v) => onChange({ ...shape, at: { ...shape.at, y: v } }))}
          </div>
        </>
      )}
      {shape.kind === 'polygon' && <p className="note">Polígono de {shape.points.length} vértices</p>}
    </div>
  );
}
