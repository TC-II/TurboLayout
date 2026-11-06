// Formulario de parámetros de un MOSFET de GF180 (para colocar uno nuevo o editar una instancia).

import { useEffect, useState } from 'react';
import type { MosParams } from './model.ts';

export const DEFAULT_MOS: MosParams = { kind: 'nfet', w: 1, l: 0.28, fingers: 1, gate_contact: 'top', guard_ring: false };

interface Props {
  value: MosParams;
  action: string;
  onSubmit: (p: MosParams) => void;
  onCancel?: () => void;
}

export function MosForm({ value, action, onSubmit, onCancel }: Props) {
  const [p, setP] = useState(value);
  useEffect(() => setP(value), [value]);
  const num = (k: 'w' | 'l', min: number, label: string) => (
    <label className="num">
      <span>{label}</span>
      <input
        type="number"
        step={0.005}
        min={min}
        value={p[k]}
        onChange={(e) => setP({ ...p, [k]: Number(e.target.value) })}
      />
    </label>
  );
  return (
    <form
      className="props"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit({ ...p, w: Math.max(p.w, 0.22), l: Math.max(p.l, 0.28), fingers: Math.max(1, Math.round(p.fingers)) });
      }}
    >
      <h3>Transistor GF180 (3.3 V)</h3>
      <label className="num">
        <span>Tipo</span>
        <select value={p.kind} onChange={(e) => setP({ ...p, kind: e.target.value as MosParams['kind'] })}>
          <option value="nfet">nfet_03v3</option>
          <option value="pfet">pfet_03v3</option>
        </select>
      </label>
      <div className="grid2">
        {num('w', 0.22, 'W por dedo (µm)')}
        {num('l', 0.28, 'L (µm)')}
        <label className="num">
          <span>Dedos</span>
          <input type="number" min={1} step={1} value={p.fingers} onChange={(e) => setP({ ...p, fingers: Number(e.target.value) })} />
        </label>
        <label className="num">
          <span>Contacto de gate</span>
          <select value={p.gate_contact} onChange={(e) => setP({ ...p, gate_contact: e.target.value as MosParams['gate_contact'] })}>
            <option value="none">ninguno</option>
            <option value="top">arriba</option>
            <option value="bottom">abajo</option>
            <option value="both">ambos</option>
          </select>
        </label>
      </div>
      <label className="check">
        <input type="checkbox" checked={p.guard_ring} onChange={(e) => setP({ ...p, guard_ring: e.target.checked })} />
        Guard ring
      </label>
      <p className="note">W total: {(p.w * Math.max(1, p.fingers)).toFixed(3)} µm</p>
      <div className="row">
        <button type="submit">{action}</button>
        {onCancel && (
          <button type="button" onClick={onCancel}>
            Cancelar
          </button>
        )}
      </div>
    </form>
  );
}
