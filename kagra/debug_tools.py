
def debug_info(db):
    print("==== AssetDatabase Debug ====")
    print("Total records:", len(db._records))

    types = {}
    for r in db._records.values():
        t = r.asset_type
        types[t] = types.get(t, 0) + 1

    print("By type:")
    for k,v in types.items():
        print(" ", k, ":", v)

    loaded = sum(1 for r in db._records.values() if r.loaded is not None)
    print("Loaded:", loaded)
    print("Unloaded:", len(db._records) - loaded)
