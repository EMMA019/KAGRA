
class AssetBrowser:

    def __init__(self, db):
        self.db = db

    def list_all(self):
        for r in self.db.list_records():
            print(f"{r.asset_type:10} | {r.key:40} | {r.path}")

    def list_type(self, asset_type):
        for r in self.db.list_records(asset_type):
            print(f"{r.asset_type:10} | {r.key:40} | {r.path}")

    def search(self, text):
        for r in self.db.list_records():
            if text in r.key:
                print(f"{r.asset_type:10} | {r.key:40} | {r.path}")

    def stats(self):
        total = len(self.db._records)
        loaded = sum(1 for r in self.db._records.values() if r.loaded is not None)
        print("Asset stats")
        print("-----------")
        print("total:", total)
        print("loaded:", loaded)
        print("unloaded:", total - loaded)
