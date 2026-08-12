import json
import struct
import sys

def analyze_vrm(path):
    with open(path, 'rb') as f:
        data = f.read()
    
    if data[0:4] != b'glTF':
        print("Not a glTF file")
        return
    
    offset = 12
    json_bytes = None
    
    while offset + 8 <= len(data):
        chunk_len = struct.unpack('<I', data[offset:offset+4])[0]
        chunk_type = struct.unpack('<I', data[offset+4:offset+8])[0]
        chunk_data = data[offset+8:offset+8+chunk_len]
        
        if chunk_type == 0x4E4F534A:  # JSON
            json_bytes = chunk_data
            break
        
        offset += 8 + chunk_len
    
    if json_bytes is None:
        print("No JSON chunk found")
        return
    
    json_str = json_bytes.decode('utf-8').rstrip('\0')
    parsed = json.loads(json_str)
    
    print("VRM analysis for:", path)
    print("=" * 80)
    
    # Check for VRM 0.x
    if 'extensions' in parsed and 'VRM' in parsed['extensions']:
        print("VRM 0.x extension found")
        vrm = parsed['extensions']['VRM']
        if 'blendShapeMaster' in vrm:
            master = vrm['blendShapeMaster']
            if 'blendShapeGroups' in master:
                groups = master['blendShapeGroups']
                print(f"  Found {len(groups)} blend shape groups:")
                for group in groups:
                    name = group.get('name', 'unnamed')
                    binds = group.get('binds', [])
                    print(f"    - {name}: {len(binds)} binds")
                    for bind in binds[:2]:
                        mesh_idx = bind.get('mesh', 0)
                        index = bind.get('index', 0)
                        weight = bind.get('weight', 1.0)
                        print(f"      mesh={mesh_idx}, index={index}, weight={weight}")
    
    # Check for VRM 1.0
    if 'extensions' in parsed and 'VRMC_vrm' in parsed['extensions']:
        print("\nVRM 1.0 extension (VRMC_vrm) found")
        vrm1 = parsed['extensions']['VRMC_vrm']
        
        if 'expressions' in vrm1:
            expressions = vrm1['expressions']
            print(f"  Found {len(expressions)} expressions:")
            for name, expr in expressions.items():
                print(f"    - {name}:")
                print(f"      Keys: {list(expr.keys())}")
                if 'morphTargetBinds' in expr:
                    binds = expr['morphTargetBinds']
                    print(f"      morphTargetBinds: {len(binds)}")
                    for i, bind in enumerate(binds[:3]):
                        print(f"        [{i}] {bind}")
                else:
                    print("      No morphTargetBinds")
                    
                if 'isBinary' in expr:
                    print(f"      isBinary: {expr['isBinary']}")
                if 'overrideBlink' in expr:
                    print(f"      overrideBlink: {expr['overrideBlink']}")
                if 'overrideLookAt' in expr:
                    print(f"      overrideLookAt: {expr['overrideLookAt']}")
                if 'overrideMouth' in expr:
                    print(f"      overrideMouth: {expr['overrideMouth']}")
        
        # Check for humanoid bone mapping
        if 'humanoid' in vrm1 and 'humanBones' in vrm1['humanoid']:
            print(f"\n  Human bones: {len(vrm1['humanoid']['humanBones'])}")
    
    # Check for meshes and their extras
    if 'meshes' in parsed:
        print(f"\nTotal meshes: {len(parsed['meshes'])}")
        for i, mesh in enumerate(parsed['meshes']):
            if 'extras' in mesh:
                extras = mesh['extras']
                if 'targetNames' in extras:
                    names = extras['targetNames']
                    print(f"  Mesh {i} has {len(names)} targetNames: {names[:5]}{'...' if len(names) > 5 else ''}")
                else:
                    print(f"  Mesh {i}: extras={list(extras.keys())}")
            else:
                print(f"  Mesh {i}: no extras")
    
    print("=" * 80)

if __name__ == '__main__':
    analyze_vrm('assets/Emma.vrm')
