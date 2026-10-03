import type { RefObject } from 'react';
import type { PlanogramSession } from './session';
import type { ScenarioView, ScenarioMetrics } from './types';

type Tone = 'better' | 'worse' | 'neutral';

/** One headline comparison. Values are Rust metrics; the delta is display arithmetic only. */
interface MetricCard {
  label: string;
  value: (m: ScenarioMetrics) => string;
  delta: (before: ScenarioMetrics, after: ScenarioMetrics) => { text: string; tone: Tone } | undefined;
}

const signed = (value: number, digits = 0) => `${value > 0 ? '+' : value < 0 ? '−' : '±'}${Math.abs(value).toFixed(digits)}`;
const tone = (change: number, higherIsBetter: boolean): Tone => change === 0 ? 'neutral' : (change > 0) === higherIsBetter ? 'better' : 'worse';
const days = (m: ScenarioMetrics) => (m.aggregate_days_supply_millidays ?? 0) / 1000;

const CARDS: MetricCard[] = [
  { label: 'Fixture', value: m => `${m.bay_count} × 4 ft`, delta: (b, a) => a.bay_count === b.bay_count ? undefined : { text: `${signed(a.bay_count - b.bay_count)} bays`, tone: 'neutral' } },
  { label: 'Distinct assortment', value: m => `${m.distinct_sku_count} / ${m.expected_sku_count} SKUs`, delta: (b, a) => a.distinct_sku_count === b.distinct_sku_count ? { text: 'All kept', tone: 'better' } : { text: `${signed(a.distinct_sku_count - b.distinct_sku_count)} SKUs`, tone: tone(a.distinct_sku_count - b.distinct_sku_count, true) } },
  { label: 'Shelf capacity', value: m => `${m.capacity_units.toLocaleString()} units`, delta: (b, a) => b.capacity_units === 0 ? undefined : { text: `${signed((a.capacity_units - b.capacity_units) / b.capacity_units * 100)}%`, tone: tone(a.capacity_units - b.capacity_units, true) } },
  { label: 'Weighted stock cover', value: m => `${days(m).toFixed(2)} days`, delta: (b, a) => ({ text: `${signed(days(a) - days(b), 2)} days`, tone: tone(days(a) - days(b), true) }) },
  { label: 'SKUs under 3 days', value: m => `${m.below_three_days_sku_count}`, delta: (b, a) => ({ text: signed(a.below_three_days_sku_count - b.below_three_days_sku_count), tone: tone(a.below_three_days_sku_count - b.below_three_days_sku_count, false) }) },
  { label: 'Stocked SKUs under 7 days', value: m => `${m.below_seven_days_sku_count}`, delta: (b, a) => ({ text: signed(a.below_seven_days_sku_count - b.below_seven_days_sku_count), tone: tone(a.below_seven_days_sku_count - b.below_seven_days_sku_count, false) }) },
  { label: 'Σ SKU capacity turnovers / week', value: m => `${(m.replenishment_turnovers_per_week_milli / 1000).toFixed(2)}`, delta: (b, a) => ({ text: signed((a.replenishment_turnovers_per_week_milli - b.replenishment_turnovers_per_week_milli) / 1000, 2), tone: 'neutral' }) },
];

export function ScenarioPanel({ scenario, sessionRef, onChanged, comparison, onComparisonChange }: {scenario:ScenarioView;sessionRef:RefObject<PlanogramSession|undefined>;onChanged:()=>void;comparison:boolean;onComparisonChange:(enabled:boolean)=>void}) {
  const {baseline,current,assumptions,products}=scenario.comparison;
  function act(operation:()=>void) {
    if(sessionRef.current?.hasPendingProposal() && !window.confirm('Discard the pending proposal and switch scenario view? Committed edits remain in their alternative.')) return;
    try {operation();onChanged();} catch(error){window.alert(String(error));}
  }
  // Rust `None` arrives as `undefined` through serde-wasm-bindgen.
  const viewingBaseline = scenario.active == null;
  return <section className="scenario-panel" aria-label="Cereal challenge comparison">
    <div className="scenario-title"><div><span className="section-label">SIMULATION · SEED {scenario.seed}</span><strong>Cereal / Eight bays into six</strong><small>Same assumed demand. Less space. Keep the assortment working.</small></div>
      <label>Viewing<select aria-label="Scenario alternative" value={scenario.active??-1} onChange={e=>act(()=>sessionRef.current?.selectAlternative(Number(e.target.value)))}><option value={-1}>Eight-bay baseline · locked</option>{scenario.alternatives.map((name,i)=><option key={i} value={i}>{name}</option>)}</select></label>
      <button className="compare-toggle" type="button" aria-pressed={comparison && !viewingBaseline} disabled={viewingBaseline} onClick={()=>onComparisonChange(!comparison)}>Compare with baseline</button>
      <button disabled={scenario.alternatives.length>=6} onClick={()=>act(()=>sessionRef.current?.duplicateAlternative())}>Duplicate target</button>
    </div>
    <ul className="scenario-metrics" aria-label={viewingBaseline ? 'Baseline metrics' : `${scenario.alternatives[scenario.active ?? 0]} compared with the eight-bay baseline`}>{CARDS.map(card => { const change = viewingBaseline ? undefined : card.delta(baseline, current); return <li key={card.label} className="metric-card"><span>{card.label}</span><strong>{card.value(current)}</strong><small>{viewingBaseline ? 'Eight-bay baseline' : `from ${card.value(baseline)}`}</small>{change && <em className={`metric-delta ${change.tone}`}>{change.text}</em>}</li>; })}</ul>
    <p className="scenario-proxy-note">Replenishment proxy = Σ (each stocked SKU’s assumed weekly demand ÷ its shelf capacity). Not category inventory turns or replenishment trips.</p>
    <div className="scenario-foot"><span>{current.validation_issue_count===0?'Physical fit valid':`${current.validation_issue_count} fit issues`} · {current.unplaced_product_ids.length} unplaced SKUs · {current.within_six_bay_limit?'Within six-bay limit':'Eight-bay reference; exceeds target'}</span><details><summary>Assumptions & SKU comparison</summary><div className="scenario-detail"><p>{assumptions.source}. Assumed category demand: {(current.weekly_demand_milliunits/1000).toFixed(3)} units/store/week.</p>{Object.entries(assumptions).filter(([k])=>!['source','generator_version'].includes(k)).map(([key,value])=><p key={key}>{value}</p>)}<table><thead><tr><th>SKU</th><th>Units/week</th><th>Capacity 8 → current</th><th>Days 8 → current</th></tr></thead><tbody>{products.map(p=><tr key={p.product_id}><th>{p.description}</th><td>{(p.weekly_demand_milliunits/1000).toFixed(2)}</td><td>{p.baseline_capacity_units} → {p.current_capacity_units}</td><td>{((p.baseline_days_supply_millidays??0)/1000).toFixed(2)} → {((p.current_days_supply_millidays??0)/1000).toFixed(2)}</td></tr>)}</tbody></table></div></details></div>
  </section>;
}
