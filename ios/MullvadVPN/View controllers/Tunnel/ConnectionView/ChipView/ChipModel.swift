//
//  FeatureChipModel.swift
//  MullvadVPN
//
//  Created by Mojgan on 2024-12-05.
//  Copyright © 2025 Mullvad VPN AB. All rights reserved.
//

import Foundation
import SwiftUI

struct ChipModel: Identifiable, Hashable {
    var id: String { name }
    let name: String
}
