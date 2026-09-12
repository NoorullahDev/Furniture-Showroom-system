use rusqlite::{Connection, OpenFlags};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("database path");
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    let shop_name: Option<String> = db
        .query_row(
            "SELECT json_extract(value_json, '$') FROM settings WHERE key = 'shop.name'",
            [],
            |row| row.get(0),
        )
        .ok();
    println!("shop.name={shop_name:?}");

    let mut locations = db.prepare(
        "SELECT l.id, l.name, l.type, l.is_active,
                COALESCE(SUM(b.on_hand), 0), COALESCE(SUM(b.reserved), 0),
                COALESCE(SUM(b.damaged), 0), COUNT(DISTINCT b.product_id),
                (SELECT COUNT(*) FROM stock_movements m WHERE m.location_id = l.id)
         FROM locations l
         LEFT JOIN stock_balances b ON b.location_id = l.id
         GROUP BY l.id
         ORDER BY l.id",
    )?;
    let rows = locations.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, i64>(8)?,
        ))
    })?;
    for row in rows {
        let (id, name, kind, active, on_hand, reserved, damaged, products, movements) = row?;
        println!(
            "location id={id} name={name:?} type={kind} active={active} products={products} on_hand={on_hand} reserved={reserved} damaged={damaged} movements={movements}"
        );
    }

    let totals: (i64, i64, i64) = db.query_row(
        "SELECT COALESCE(SUM(on_hand),0), COALESCE(SUM(reserved),0), COALESCE(SUM(damaged),0) FROM stock_balances",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    println!("balance_totals on_hand={} reserved={} damaged={}", totals.0, totals.1, totals.2);

    let valuation: (i64, i64) = db.query_row(
        "SELECT COALESCE(SUM(quantity),0), COALESCE(SUM(quantity * unit_cost_minor),0) FROM inventory_cost_layers",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    println!("cost_layers quantity={} value_minor={}", valuation.0, valuation.1);

    let mut refs = db.prepare(
        "SELECT m.name, fk.[from]
           FROM sqlite_master m, pragma_foreign_key_list(m.name) fk
          WHERE m.type = 'table' AND fk.[table] = 'locations'
          ORDER BY m.name",
    )?;
    let linked = refs.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    for link in linked {
        let (table, column) = link?;
        let sql = format!(
            "SELECT COUNT(*), COUNT(DISTINCT {column}) FROM {table} WHERE {column} IS NOT NULL"
        );
        let counts: (i64, i64) = db.query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?)))?;
        println!("reference {table}.{column} rows={} locations={}", counts.0, counts.1);
        let grouped_sql = format!(
            "SELECT l.id, l.name, COUNT(*) FROM {table} t JOIN locations l ON l.id = t.{column} GROUP BY l.id ORDER BY l.id"
        );
        let mut grouped = db.prepare(&grouped_sql)?;
        let groups = grouped.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?;
        for group in groups {
            let (id, name, count) = group?;
            println!("  location id={id} name={name:?} rows={count}");
        }
    }

    Ok(())
}
