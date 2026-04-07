
import tkinter as tk

class SceneView(tk.Canvas):
    def __init__(self, master):
        super().__init__(master, bg="#222")
        self.entities = []

    def set_scene(self, root):
        self.entities=[]
        def walk(e):
            self.entities.append(e)
            for c in e.children:
                walk(c)
        walk(root)

    def redraw(self):
        self.delete("all")
        for e in self.entities:
            t = e.components.get("Transform")
            if not t: continue
            x,y,_ = t["position"]
            self.create_rectangle(x*50+300, y*50+200, x*50+320, y*50+220, outline="white")
