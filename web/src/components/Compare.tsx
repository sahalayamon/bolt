import { COMPARE_COLUMNS, COMPARE_ROWS } from '../data';
import { Reveal } from './Reveal';

export function Compare() {
  return (
    <section className="section" id="compare">
      <div className="wrap">
        <Reveal as="p" className="eyebrow">
          Bolt vs the usual ways
        </Reveal>
        <Reveal as="h2" className="h2">
          No internet. No account.
          <br />
          <span className="muted">No one in the middle.</span>
        </Reveal>
        <Reveal className="cmp-scroll">
          <table className="cmp">
            <thead>
              <tr>
                <th scope="col" />
                {COMPARE_COLUMNS.map((c, i) => (
                  <th key={c} scope="col" className={i === 0 ? 'cmp-bolt' : undefined}>
                    {c}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {COMPARE_ROWS.map(({ label, values }) => (
                <tr key={label}>
                  <th scope="row">{label}</th>
                  {values.map((v, i) => (
                    <td key={COMPARE_COLUMNS[i]} className={i === 0 ? 'cmp-bolt' : undefined}>
                      {v}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </Reveal>
        <Reveal as="p" className="cmp-note">
          Bolt has one trade-off: both devices need to be on the same Wi-Fi, hotspot or USB tether.
        </Reveal>
      </div>
    </section>
  );
}
