import type { RefObject } from 'react';
import type { PlanogramSession } from './session';
import type { ScenarioView, ScenarioMetrics } from './types';

export function ScenarioPanel({ scenario, sessionRef, onChanged }: {scenario:ScenarioView;sessionRef:RefObject<PlanogramSession|undefined>;onChanged:()=>void}) {
  const {baseline,current,assumptions,products}=scenario.comparison;
  function act(operation:()=>void) {
    if(sessionRef.current?.hasPendingProposal() && !window.confirm('Discard the pending proposal and switch scenario view? Committed edits remain in their alternative.')) return;
    try {operation();onChanged();} catch(error){window.alert(String(error));}
  }
  const rows:[string,(m:ScenarioMetrics)=>string][]=[
    ['Bays',m=>`${m.bay_count} × 4 ft`],
    ['Distinct assortment',m=>`${m.distinct_sku_count} / ${m.expected_sku_count} SKUs`],
    ['Shelf capacity',m=>`${m.capacity_units.toLocaleString()} units`],
    ['Weighted stock cover',m=>`${((m.aggregate_days_supply_millidays??0)/1000).toFixed(2)} days`],
    ['Stocked SKUs under 7 days',m=>`${m.below_seven_days_sku_count}`],
    ['Σ SKU capacity turnovers / week',m=>`${(m.replenishment_turnovers_per_week_milli/1000).toFixed(2)}`],
  ];
  return <section className="scenario-panel" aria-label="Cereal challenge comparison">
    <div className="scenario-title"><div><span className="section-label">SIMULATION · SEED {scenario.seed}</span><strong>Cereal / Eight bays into six</strong><small>Same assumed demand. Less space. Keep the assortment working.</small></div>
      <label>Viewing<select aria-label="Scenario alternative" value={scenario.active??-1} onChange={e=>act(()=>sessionRef.current?.selectAlternative(Number(e.target.value)))}><option value={-1}>Eight-bay baseline · locked</option>{scenario.alternatives.map((name,i)=><option key={i} value={i}>{name}</option>)}</select></label>
      <button disabled={scenario.alternatives.length>=6} onClick={()=>act(()=>sessionRef.current?.duplicateAlternative())}>Duplicate target</button>
    </div>
    <div className="scenario-metrics"><div className="metric-labels"><span>Compared with</span><strong>8-bay baseline</strong><strong>{scenario.active===null?'Baseline view':'Current alternative'}</strong></div>{rows.map(([label,value])=><div key={label}><span>{label}</span><strong>{value(baseline)}</strong><strong>{value(current)}</strong></div>)}</div>
    <p className="scenario-proxy-note">Replenishment proxy = Σ (each stocked SKU’s assumed weekly demand ÷ its shelf capacity). Not category inventory turns or replenishment trips.</p>
    <div className="scenario-foot"><span>{current.validation_issue_count===0?'Physical fit valid':`${current.validation_issue_count} fit issues`} · {current.unplaced_product_ids.length} unplaced SKUs · {current.within_six_bay_limit?'Within six-bay limit':'Eight-bay reference; exceeds target'}</span><details><summary>Assumptions & SKU comparison</summary><div className="scenario-detail"><p>{assumptions.source}. Assumed category demand: {(current.weekly_demand_milliunits/1000).toFixed(3)} units/store/week.</p>{Object.entries(assumptions).filter(([k])=>!['source','generator_version'].includes(k)).map(([key,value])=><p key={key}>{value}</p>)}<table><thead><tr><th>SKU</th><th>Units/week</th><th>Capacity 8 → current</th><th>Days 8 → current</th></tr></thead><tbody>{products.map(p=><tr key={p.product_id}><th>{p.description}</th><td>{(p.weekly_demand_milliunits/1000).toFixed(2)}</td><td>{p.baseline_capacity_units} → {p.current_capacity_units}</td><td>{((p.baseline_days_supply_millidays??0)/1000).toFixed(2)} → {((p.current_days_supply_millidays??0)/1000).toFixed(2)}</td></tr>)}</tbody></table></div></details></div>
  </section>;
}
